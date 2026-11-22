//! `GET /ws/communities/{id}`: a member's live feed of a community: who is in
//! which voice channel, text channel messages and structure changes, for the
//! channels the member may view.

use super::feed_messages::{FeedMessage, RemovedReason, VoiceEntryDto};
use super::messages::CloseReason;
use super::writer::{Outbound, finish, spawn_writer};
use crate::AppState;
use crate::http::{Authenticated, authenticate, community_response, from_foreign_origin};
use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum_extra::extract::cookie::CookieJar;
use futures_util::StreamExt;
use melior_application::{CommunitiesError, CommunityEvent, FeedFilter, FeedVerdict, SessionState};
use melior_domain::{AuthSessionId, CommunityId, User};
use tokio::sync::broadcast::error::RecvError;
use tokio::time::MissedTickBehavior;
use tracing::debug;

/// The feed accepts no messages but closes; anything else small is ignored.
const MAX_CLIENT_MESSAGE_BYTES: usize = 1024;

pub fn routes() -> Router<AppState> {
    Router::new().route("/ws/communities/{id}", get(upgrade))
}

/// Origin, session, then membership (404 unknown, 403 not a member) before any
/// WebSocket exists.
async fn upgrade(
    Path(id): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    upgrade: WebSocketUpgrade,
) -> Response {
    if from_foreign_origin(&state, &headers) {
        return (StatusCode::FORBIDDEN, "origin not allowed").into_response();
    }
    let Authenticated { user, session_id, jar } = match authenticate(&state, jar).await {
        Ok(authenticated) => authenticated,
        Err((rejection, jar)) => return (jar, rejection.status()).into_response(),
    };
    let Some(community_id) = CommunityId::parse(&id) else {
        return (jar, (StatusCode::BAD_REQUEST, "invalid community id")).into_response();
    };
    let filter = match state.communities.feed_filter(&community_id, &user).await {
        Ok(filter) => filter,
        Err(error) => return (jar, community_response(&error)).into_response(),
    };
    let member = Member { state, auth_session: session_id, user, community_id, filter };
    let response = upgrade
        .max_message_size(MAX_CLIENT_MESSAGE_BYTES)
        .on_upgrade(move |socket| connection(socket, member));
    (jar, response).into_response()
}

/// Whose feed it is, and what it may carry.
struct Member {
    state: AppState,
    auth_session: AuthSessionId,
    user: User,
    community_id: CommunityId,
    filter: FeedFilter,
}

/// Whether the feed goes on after a step.
enum Step {
    Continue,
    End,
}

type Outbox = Outbound;

fn push(outbox: &Outbox, message: FeedMessage) {
    outbox.push(message);
}

/// Runs one feed: subscribe first so nothing is missed, then the snapshot, then
/// events, re-checks and the client's close.
async fn connection(socket: WebSocket, mut member: Member) {
    let (sink, mut stream) = socket.split();
    let (outbox, writer) = spawn_writer(sink);
    let mut events = member.state.communities.events().subscribe(&member.community_id);
    member.snapshot(&outbox).await;

    let period = member.state.session_recheck;
    let mut recheck = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
    recheck.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        let step = tokio::select! {
            message = stream.next() => match message {
                Some(Ok(Message::Close(_)) | Err(_)) | None => Step::End,
                Some(Ok(_)) => Step::Continue,
            },
            () = outbox.overflowed() => {
                debug!("feed client is not reading; closing the feed");
                Step::End
            }
            event = events.recv() => member.on_event(event, &outbox).await,
            _ = recheck.tick() => member.recheck(&outbox).await,
        };
        if matches!(step, Step::End) {
            break;
        }
    }
    drop(outbox);
    finish(writer).await;
}

impl Member {
    async fn on_event(
        &mut self,
        event: Result<CommunityEvent, RecvError>,
        outbox: &Outbox,
    ) -> Step {
        let event = match event {
            Ok(event) => event,
            Err(RecvError::Lagged(missed)) => {
                debug!(missed, "feed fell behind; resynchronising");
                push(outbox, FeedMessage::StructureChanged {});
                return self.recheck(outbox).await;
            }
            Err(RecvError::Closed) => return Step::End,
        };
        match self.filter.verdict(&event) {
            FeedVerdict::Drop => Step::Continue,
            FeedVerdict::Forward => {
                if let Some(message) = FeedMessage::forwarded(event) {
                    push(outbox, message);
                }
                Step::Continue
            }
            FeedVerdict::ForwardAndRefresh => {
                push(outbox, FeedMessage::StructureChanged {});
                self.refresh(outbox).await
            }
            FeedVerdict::End(end) => {
                push(outbox, FeedMessage::Removed { reason: end.into() });
                Step::End
            }
        }
    }

    /// The session, then membership and the visible channels, then a fresh
    /// snapshot. Only a definite answer ends the feed.
    async fn recheck(&mut self, outbox: &Outbox) -> Step {
        match self.state.auth.check(&self.auth_session).await {
            Ok(SessionState::Active(active)) => self.user = active.user,
            Ok(SessionState::Ended) => {
                push(outbox, FeedMessage::Closed { reason: CloseReason::SessionEnded });
                return Step::End;
            }
            Ok(SessionState::Unverifiable) => {}
            Err(error) => debug!(%error, "feed session re-check failed"),
        }
        if matches!(self.refresh(outbox).await, Step::End) {
            return Step::End;
        }
        self.snapshot(outbox).await;
        Step::Continue
    }

    /// Takes the channels the member may view now.
    async fn refresh(&mut self, outbox: &Outbox) -> Step {
        match self.state.communities.feed_filter(&self.community_id, &self.user).await {
            Ok(filter) => self.filter = filter,
            Err(CommunitiesError::Store(error)) => debug!(%error, "feed re-check failed"),
            Err(CommunitiesError::NotFound) => {
                push(outbox, FeedMessage::Removed { reason: RemovedReason::Deleted });
                return Step::End;
            }
            Err(_) => {
                push(outbox, FeedMessage::Removed { reason: RemovedReason::Kicked });
                return Step::End;
            }
        }
        Step::Continue
    }

    async fn snapshot(&self, outbox: &Outbox) {
        match self.state.communities.voice_roster(&self.community_id).await {
            Ok(roster) => {
                let voice = self.filter.snapshot(roster).into_iter().map(VoiceEntryDto::from);
                push(outbox, FeedMessage::Snapshot { voice: voice.collect() });
            }
            Err(error) => debug!(%error, "voice roster unavailable"),
        }
    }
}
