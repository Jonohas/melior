use super::chat::ChatSendDto;
use super::limiter::RelayLimits;
use super::messages::{ClientMessage, CloseReason, SdpType, ServerMessage};
use super::peer::{PeerCommand, Rejection};
use super::writer::{finish, spawn_writer};
use crate::AppState;
use crate::http::{Authenticated, authenticate, community_response, from_foreign_origin};
use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum_extra::extract::cookie::CookieJar;
use futures_util::StreamExt;
use melior_application::{
    Admission, ChatFailure, ChatService, CommunitiesError, ConferenceError, ConferenceService,
    PresenceContext, SessionState, Signal, SignalSink, VoiceGrant,
};
use melior_domain::{AuthSessionId, ChannelId, DomainError, Permissions, SessionId, User};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;
use tracing::{debug, info, warn};

/// The largest WebSocket message the server accepts, above the largest SDP it relays.
pub const MAX_MESSAGE_BYTES: usize = 128 * 1024;

/// `GET /ws?channel_id=<id>`: conference signalling in a voice channel.
pub fn routes() -> Router<AppState> {
    Router::new().route("/ws", get(upgrade))
}

#[derive(Deserialize)]
struct ConnectParams {
    channel_id: Option<String>,
}

/// Checks who is asking before any WebSocket exists. In order: a browser must be
/// on the application's own page; the session cookie must belong to a user with a
/// Melior role; the channel must be a voice channel the user may view and connect
/// to (404 unknown, 403 refused); this server instance must be the one hosting the
/// room (503); and the room must have a free seat (409).
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

    let Some(channel_id) = params.channel_id else {
        return (jar, (StatusCode::BAD_REQUEST, "channel_id is required")).into_response();
    };
    let channel_id = match ChannelId::parse(&channel_id) {
        Ok(channel_id) => channel_id,
        Err(error) => return (jar, (StatusCode::BAD_REQUEST, error.to_string())).into_response(),
    };

    let grant = match state.communities.admit_voice(&channel_id, &user).await {
        Ok(grant) => grant,
        Err(error) => return (jar, community_response(&error)).into_response(),
    };

    // 503 either way: the client retries, and its next attempt is routed to the
    // instance that hosts the room, or lands here again once the lease is free.
    if let Err(error) = state.conference.host(&channel_id).await {
        if !matches!(error, ConferenceError::HostedElsewhere) {
            warn!(%error, channel = %channel_id, "room lease unavailable");
        }
        return (jar, StatusCode::SERVICE_UNAVAILABLE).into_response();
    }
    if state.conference.is_full(&channel_id, grant.plan(state.conference.mesh())).await {
        return (jar, (StatusCode::CONFLICT, "this channel is full")).into_response();
    }

    let guest = Guest { state: state.clone(), auth_session: session_id, channel_id, user, grant };
    let response = upgrade
        .max_message_size(MAX_MESSAGE_BYTES)
        .max_frame_size(MAX_MESSAGE_BYTES)
        .on_upgrade(move |socket| connection(socket, guest));
    (jar, response).into_response()
}

/// Who a connection belongs to, and what it takes for it to stay open.
struct Guest {
    state: AppState,
    auth_session: AuthSessionId,
    channel_id: ChannelId,
    user: User,
    /// What the user may do in the channel, as of the last check.
    grant: VoiceGrant,
}

impl Guest {
    /// How the user joins the channel's room.
    fn admission(&self) -> Admission {
        Admission {
            channel_id: self.channel_id.clone(),
            plan: self.grant.plan(self.state.conference.mesh()),
            presence: Some(PresenceContext {
                community_id: self.grant.community_id.clone(),
                user_id: self.user.id.clone(),
            }),
        }
    }

    /// Re-checks everything the upgrade checked except the capacity, and takes the
    /// user's current permissions. Returns why the connection has to end, if it
    /// does. Only a definite answer ends it: when the session store, the community
    /// store or the lease cannot be asked, the connection stays.
    async fn overstayed(&mut self) -> Option<CloseReason> {
        match self.state.auth.check(&self.auth_session).await {
            Ok(SessionState::Active(active)) => self.user = active.user,
            Ok(SessionState::Ended) => return Some(CloseReason::SessionEnded),
            Ok(SessionState::Unverifiable) => {}
            Err(error) => debug!(%error, "session re-check failed"),
        }

        match self.state.communities.admit_voice(&self.channel_id, &self.user).await {
            Ok(grant) => self.grant = grant,
            Err(CommunitiesError::Store(error)) => debug!(%error, "channel access re-check failed"),
            Err(_) => return Some(CloseReason::Removed),
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
    let (sink, mut stream) = socket.split();
    let (tx, writer) = spawn_writer::<ServerMessage>(sink);

    let outbox = Arc::new(Outbox(tx));
    let service = guest.state.conference.clone();
    let chat = guest.state.chat.clone();
    match service.join(guest.admission(), &guest.user, outbox.clone()).await {
        Ok(session_id) => {
            let period = guest.state.session_recheck;
            let mut recheck =
                tokio::time::interval_at(tokio::time::Instant::now() + period, period);
            recheck.set_missed_tick_behavior(MissedTickBehavior::Delay);

            let mut limits = RelayLimits::new(Instant::now());

            // The session id is bound to this socket: nothing a client sends can
            // make its messages act on another participant.
            loop {
                tokio::select! {
                    message = stream.next() => match message {
                        Some(Ok(Message::Text(text))) => {
                            let services = Services {
                                conference: &service,
                                chat: &chat,
                                permissions: guest.grant.permissions,
                            };
                            dispatch(&services, &session_id, &outbox, &mut limits, text.as_str())
                                .await;
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
                        if let Err(error) = service.refresh_presence(&session_id).await {
                            debug!(%error, "presence refresh failed");
                        }
                    }
                }
            }
            service.leave(&session_id).await;
            chat.forget(&session_id);
        }
        Err(error) => {
            warn!(%error, "join failed");
            outbox.push(ServerMessage::Error { message: format!("could not join: {error}") });
        }
    }

    // Dropping the last sender lets the writer flush what is queued and close.
    drop(outbox);
    finish(writer).await;
}

/// The use cases a client message can call.
struct Services<'a> {
    conference: &'a ConferenceService,
    chat: &'a ChatService,
    /// What the participant may do in the channel.
    permissions: Permissions,
}

/// Translates one client message into a use case call.
async fn dispatch(
    services: &Services<'_>,
    session_id: &SessionId,
    outbox: &Outbox,
    limits: &mut RelayLimits,
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

    if !limits.allows(&message, Instant::now()) {
        debug!(session = %session_id, "relay message over the rate limit dropped");
        return;
    }

    let service = services.conference;
    let result = match message {
        ClientMessage::SdpOffer(description) => match description.into_sdp(SdpType::Offer) {
            Some(offer) => service.handle_offer(session_id, offer).await,
            None => return wrong_type(outbox, "sdp_offer", "offer"),
        },
        ClientMessage::SdpAnswer(description) => match description.into_sdp(SdpType::Answer) {
            Some(answer) => service.handle_answer(session_id, answer).await,
            None => return wrong_type(outbox, "sdp_answer", "answer"),
        },
        ClientMessage::Ice(candidate) => match candidate.into_checked() {
            Ok(candidate) => service.add_ice_candidate(session_id, candidate).await,
            Err(rejection) => {
                debug!(session = %session_id, ?rejection, "ice candidate dropped");
                Ok(())
            }
        },
        ClientMessage::RouteLive(dto) => peer(service, session_id, dto.validate()).await,
        ClientMessage::LinkFailed(dto) => peer(service, session_id, dto.validate()).await,
        ClientMessage::PeerOffer(dto) => peer(service, session_id, dto.validate_offer()).await,
        ClientMessage::PeerAnswer(dto) => peer(service, session_id, dto.validate_answer()).await,
        ClientMessage::PeerIce(dto) => peer(service, session_id, dto.validate()).await,
        ClientMessage::E2ee(dto) => peer(service, session_id, dto.validate()).await,
        ClientMessage::Echo(text) => {
            outbox.push(ServerMessage::Echo(text));
            Ok(())
        }
        ClientMessage::ChatSend(message) => {
            return say(services, session_id, outbox, message).await;
        }
        ClientMessage::VoiceState(voice) => service.set_voice(session_id, voice.into()).await,
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
        PeerCommand::Key { to, envelope } => {
            service.relay_key(session_id, &to, envelope).await;
            Ok(())
        }
    }
}

/// Hands a chat message to the room. A refusal goes back to the sender alone.
async fn say(
    services: &Services<'_>,
    session_id: &SessionId,
    outbox: &Outbox,
    message: ChatSendDto,
) {
    let ChatSendDto { client_id, text } = message;
    let sent = services.chat.send(session_id, client_id.clone(), &text, services.permissions);
    let Err(error) = sent.await else {
        return;
    };
    let message = match &error {
        ChatFailure::Rejected(rule) => rule.to_string(),
        ChatFailure::Conference(_) | ChatFailure::Store(_) | ChatFailure::Community(_) => {
            warn!(session = %session_id, %error, "chat message not delivered");
            "the message could not be sent, try again".to_owned()
        }
    };
    outbox.push(ServerMessage::ChatError { client_id, message });
}

fn wrong_type(outbox: &Outbox, channel: &str, expected: &str) {
    outbox.push(ServerMessage::Error {
        message: format!("{channel} payload must have type \"{expected}\""),
    });
}

fn report(session_id: &SessionId, outbox: &Outbox, error: &ConferenceError) {
    // An offer that crossed the server's own on the wire is not a fault. The client gives up
    // its offer when the server's arrives, and offers again once the exchange is over.
    if matches!(error, ConferenceError::Domain(DomainError::OfferDuringRenegotiation)) {
        debug!(session = %session_id, "client offer ignored: a server offer is outstanding");
        return;
    }
    warn!(session = %session_id, %error, "request failed");
    outbox.push(ServerMessage::Error { message: error.to_string() });
}
