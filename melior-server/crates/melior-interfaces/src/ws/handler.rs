use super::messages::{ClientMessage, CloseReason, SdpType, ServerMessage};
use super::peer::{PeerCommand, Rejection};
use crate::AppState;
use crate::http::{Authenticated, authenticate, from_foreign_origin};
use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum_extra::extract::cookie::CookieJar;
use futures_util::{SinkExt, StreamExt};
use melior_application::{
    ConferenceError, ConferenceService, RoomsError, SessionState, Signal, SignalSink,
};
use melior_domain::{AuthSessionId, ChannelId, SessionId, User};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;
use tracing::{debug, info, warn};

const DEFAULT_CHANNEL: &str = "general";

/// `GET /ws?channel_id=<room>`: conference signalling for logged-in users.
pub fn routes() -> Router<AppState> {
    Router::new().route("/ws", get(upgrade))
}

#[derive(Deserialize)]
struct ConnectParams {
    channel_id: Option<String>,
}

/// Checks who is asking before any WebSocket exists. In order: a browser must be
/// on the application's own page; the session cookie must belong to a user with a
/// Melior role; the user must be allowed into this room; and this server instance
/// must be the one hosting the room.
async fn upgrade(
    Query(params): Query<ConnectParams>,
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

    let channel_id = params.channel_id.as_deref().unwrap_or(DEFAULT_CHANNEL);
    let channel_id = match ChannelId::parse(channel_id) {
        Ok(channel_id) => channel_id,
        Err(error) => return (jar, (StatusCode::BAD_REQUEST, error.to_string())).into_response(),
    };

    match state.rooms.admit(&channel_id, &user).await {
        Ok(()) => {}
        Err(RoomsError::Forbidden) => return (jar, StatusCode::FORBIDDEN).into_response(),
        Err(error) => {
            warn!(%error, channel = %channel_id, "room registry unavailable");
            return (jar, StatusCode::SERVICE_UNAVAILABLE).into_response();
        }
    }

    // 503 either way: the client retries, and its next attempt is routed to the
    // instance that hosts the room, or lands here again once the lease is free.
    if let Err(error) = state.conference.host(&channel_id).await {
        if !matches!(error, ConferenceError::HostedElsewhere) {
            warn!(%error, channel = %channel_id, "room lease unavailable");
        }
        return (jar, StatusCode::SERVICE_UNAVAILABLE).into_response();
    }

    let guest = Guest { state: state.clone(), auth_session: session_id, channel_id, user };
    let response = upgrade.on_upgrade(move |socket| connection(socket, guest));
    (jar, response).into_response()
}

/// Who a connection belongs to, and what it takes for it to stay open.
struct Guest {
    state: AppState,
    auth_session: AuthSessionId,
    channel_id: ChannelId,
    user: User,
}

impl Guest {
    /// Re-checks everything the upgrade checked. Returns why the connection has to
    /// end, if it does. Only a definite answer ends it: when the session store, the
    /// registry or the lease cannot be asked, the connection stays.
    async fn overstayed(&mut self) -> Option<CloseReason> {
        match self.state.auth.check(&self.auth_session).await {
            Ok(SessionState::Active(active)) => self.user = active.user,
            Ok(SessionState::Ended) => return Some(CloseReason::SessionEnded),
            Ok(SessionState::Unverifiable) => {}
            Err(error) => debug!(%error, "session re-check failed"),
        }

        match self.state.rooms.may_stay(&self.channel_id, &self.user).await {
            Ok(true) => {}
            Ok(false) => return Some(CloseReason::Removed),
            Err(error) => debug!(%error, "room access re-check failed"),
        }

        match self.state.conference.host(&self.channel_id).await {
            Ok(()) => {}
            Err(ConferenceError::HostedElsewhere) => return Some(CloseReason::RoomMoved),
            Err(error) => debug!(%error, "room lease renewal failed"),
        }
        None
    }
}

/// Queues outbound messages for one socket. The queue is unbounded so that use cases
/// never wait on a slow client; signalling traffic per participant is small.
struct Outbox(mpsc::UnboundedSender<ServerMessage>);

impl Outbox {
    fn push(&self, message: ServerMessage) {
        // Fails only once the socket is gone, when there is nobody left to tell.
        let _ = self.0.send(message);
    }
}

impl SignalSink for Outbox {
    fn send(&self, signal: Signal) {
        self.push(signal.into());
    }
}

/// Runs one participant's connection from join to leave.
async fn connection(socket: WebSocket, mut guest: Guest) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();

    let mut writer = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            let text = match serde_json::to_string(&message) {
                Ok(text) => text,
                Err(error) => {
                    warn!(%error, "unserializable message");
                    continue;
                }
            };
            if sink.send(Message::text(text)).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });

    let outbox = Arc::new(Outbox(tx));
    let service = guest.state.conference.clone();
    match service.join(guest.channel_id.clone(), &guest.user, outbox.clone()).await {
        Ok(session_id) => {
            let period = guest.state.session_recheck;
            let mut recheck =
                tokio::time::interval_at(tokio::time::Instant::now() + period, period);
            recheck.set_missed_tick_behavior(MissedTickBehavior::Delay);

            // The session id is bound to this socket: nothing a client sends can
            // make its messages act on another participant.
            loop {
                tokio::select! {
                    message = stream.next() => match message {
                        Some(Ok(Message::Text(text))) => {
                            dispatch(&service, &session_id, &outbox, text.as_str()).await;
                        }
                        Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                        Some(Ok(_)) => {}
                    },
                    _ = recheck.tick() => {
                        if let Some(reason) = guest.overstayed().await {
                            info!(session = %session_id, user = %guest.user.username, ?reason, "closing connection");
                            outbox.push(ServerMessage::Closed { reason });
                            break;
                        }
                    }
                }
            }
            service.leave(&session_id).await;
        }
        Err(error) => {
            warn!(%error, "join failed");
            outbox.push(ServerMessage::Error { message: format!("could not join: {error}") });
        }
    }

    // Dropping the last sender lets the writer flush what is queued and close.
    drop(outbox);
    if tokio::time::timeout(Duration::from_secs(5), &mut writer).await.is_err() {
        writer.abort();
    }
}

/// Translates one client message into a use case call.
async fn dispatch(
    service: &ConferenceService,
    session_id: &SessionId,
    outbox: &Outbox,
    text: &str,
) {
    let message = match serde_json::from_str::<ClientMessage>(text) {
        Ok(message) => message,
        Err(error) => {
            debug!(session = %session_id, %error, "invalid message");
            outbox.push(ServerMessage::Error { message: format!("invalid message: {error}") });
            return;
        }
    };

    let result = match message {
        ClientMessage::SdpOffer(description) => match description.into_sdp(SdpType::Offer) {
            Some(offer) => service.handle_offer(session_id, offer).await,
            None => return wrong_type(outbox, "sdp_offer", "offer"),
        },
        ClientMessage::SdpAnswer(description) => match description.into_sdp(SdpType::Answer) {
            Some(answer) => service.handle_answer(session_id, answer).await,
            None => return wrong_type(outbox, "sdp_answer", "answer"),
        },
        ClientMessage::Ice(candidate) => {
            service.add_ice_candidate(session_id, candidate.into()).await
        }
        ClientMessage::RouteLive(dto) => peer(service, session_id, dto.validate()).await,
        ClientMessage::LinkFailed(dto) => peer(service, session_id, dto.validate()).await,
        ClientMessage::PeerOffer(dto) => peer(service, session_id, dto.validate_offer()).await,
        ClientMessage::PeerAnswer(dto) => peer(service, session_id, dto.validate_answer()).await,
        ClientMessage::PeerIce(dto) => peer(service, session_id, dto.validate()).await,
        ClientMessage::Echo(text) => {
            outbox.push(ServerMessage::Echo(text));
            Ok(())
        }
    };

    if let Err(error) = result {
        report(session_id, outbox, &error);
    }
}

/// Carries out a validated mesh message. A rejected one is dropped with a debug log
/// and no reply: the client tears everything down on `error`.
async fn peer(
    service: &ConferenceService,
    session_id: &SessionId,
    command: Result<PeerCommand, Rejection>,
) -> Result<(), ConferenceError> {
    let command = match command {
        Ok(command) => command,
        Err(rejection) => {
            debug!(session = %session_id, ?rejection, "mesh message dropped");
            return Ok(());
        }
    };
    match command {
        PeerCommand::RouteLive { peer, epoch } => {
            service.route_live(session_id, &peer, epoch).await
        }
        PeerCommand::LinkFailed { peer } => service.link_failed(session_id, &peer).await,
        PeerCommand::Offer { to, sdp } => {
            service.relay_peer_offer(session_id, &to, sdp).await;
            Ok(())
        }
        PeerCommand::Answer { to, sdp } => {
            service.relay_peer_answer(session_id, &to, sdp).await;
            Ok(())
        }
        PeerCommand::Ice { to, candidate } => {
            service.relay_peer_ice(session_id, &to, candidate).await;
            Ok(())
        }
    }
}

fn wrong_type(outbox: &Outbox, channel: &str, expected: &str) {
    outbox.push(ServerMessage::Error {
        message: format!("{channel} payload must have type \"{expected}\""),
    });
}

fn report(session_id: &SessionId, outbox: &Outbox, error: &ConferenceError) {
    warn!(session = %session_id, %error, "request failed");
    outbox.push(ServerMessage::Error { message: error.to_string() });
}
