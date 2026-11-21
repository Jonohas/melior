//! A real server on a random port, with the outside world replaced: an identity
//! provider that accepts anyone and sessions kept in memory.
#![allow(dead_code)]

pub mod client;

use async_trait::async_trait;
use futures_util::StreamExt;
use melior_application::{
    AuthService, AuthSettings, Authentication, AuthorizationRequest, ChatService,
    CommunityRepository, CommunityService, ConferenceService, IdentityProvider, IdpError,
    InMemoryChatRepository, InMemoryCommunityEvents, InMemoryCommunityRepository,
    InMemorySessionStore, InMemoryVoiceRoster, LeaseError, PresenceQueue, RoomLeases,
    StaticIceServers, Tokens,
};
use melior_domain::{
    Channel, ChannelId, ChannelKind, ChannelMedia, CommunityId, CommunityState, Invite,
    InviteSettings, Member, MeshPolicy, Name, Topic, User, UserId,
};
use melior_infrastructure::webrtc::WebRtcMediaEngine;
use melior_interfaces::{AppState, router};
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::handshake::client::Request;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use webrtc::api::APIBuilder;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::setting_engine::SettingEngine;
use webrtc::ice::network_type::NetworkType;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::configuration::RTCConfiguration;

/// How long a test waits for any one message or event.
pub const STEP: Duration = Duration::from_secs(20);

pub const SESSION_COOKIE: &str = "melior_session_id";
/// The origin the server believes the application is served from.
pub const FRONTEND: &str = "http://app.test";

/// Vouches for whoever the authorization code names: `"<username>:<role>,<role>"`.
pub struct FakeIdp;

#[async_trait]
impl IdentityProvider for FakeIdp {
    async fn authorization_request(
        &self,
        redirect_uri: &str,
        state: &str,
    ) -> Result<AuthorizationRequest, IdpError> {
        Ok(AuthorizationRequest {
            url: format!("http://idp.test/auth?redirect_uri={redirect_uri}&state={state}"),
            code_verifier: "verifier".into(),
            nonce: "nonce".into(),
        })
    }

    async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
        nonce: &str,
        _redirect_uri: &str,
    ) -> Result<Authentication, IdpError> {
        let (username, roles) = code.split_once(':').ok_or(IdpError::InvalidGrant)?;
        if code_verifier != "verifier" || nonce != "nonce" {
            return Err(IdpError::InvalidGrant);
        }
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        Ok(Authentication {
            user: User::new(
                UserId::new(format!("sub-{username}")),
                username,
                Some(format!("{username}@example.test")),
                roles.split(',').filter(|role| !role.is_empty()).map(str::to_owned),
            ),
            tokens: Tokens {
                access_token: format!("access-{username}"),
                refresh_token: None,
                id_token: Some(format!("id-{username}")),
                access_token_expires_at: now + 3600,
            },
        })
    }

    async fn refresh(&self, _refresh_token: &str) -> Result<Authentication, IdpError> {
        Err(IdpError::InvalidGrant)
    }

    async fn logout_url(
        &self,
        id_token: &str,
        post_logout_redirect_uri: &str,
    ) -> Result<String, IdpError> {
        Ok(format!("http://idp.test/logout?hint={id_token}&to={post_logout_redirect_uri}"))
    }
}

/// Stands in for the lease on Redis: the test decides whether this instance still
/// hosts its rooms.
#[derive(Default)]
pub struct SwitchableLeases {
    hosted_elsewhere: AtomicBool,
}

impl SwitchableLeases {
    /// Simulates another instance taking over every room.
    pub fn move_rooms_elsewhere(&self) {
        self.hosted_elsewhere.store(true, Ordering::SeqCst);
    }
}

#[async_trait]
impl RoomLeases for SwitchableLeases {
    async fn hold(&self, _channel_id: &ChannelId) -> Result<bool, LeaseError> {
        Ok(!self.hosted_elsewhere.load(Ordering::SeqCst))
    }
}

/// How often the test server re-checks open connections.
pub const RECHECK: Duration = Duration::from_millis(150);

/// How long a voice roster entry lives without a refresh: three re-checks.
pub const ROSTER_TTL: Duration = Duration::from_millis(450);

pub struct TestServer {
    pub address: SocketAddr,
    pub auth: Arc<AuthService>,
    pub sessions: Arc<InMemorySessionStore>,
    pub communities: Arc<CommunityService>,
    pub repository: Arc<InMemoryCommunityRepository>,
    pub events: Arc<InMemoryCommunityEvents>,
    pub leases: Arc<SwitchableLeases>,
    /// A community every user who logs in with a role joins, owned by nobody who
    /// logs in. Voice channels named by the media tests live here.
    pub lobby: CommunityId,
}

impl TestServer {
    pub async fn start() -> Self {
        Self::start_with_mesh(MeshPolicy::OFF).await
    }

    /// A server whose rooms run as a mesh up to the policy's limit.
    pub async fn start_with_mesh(mesh: MeshPolicy) -> Self {
        let media = Arc::new(WebRtcMediaEngine::new().expect("media engine"));
        let leases = Arc::new(SwitchableLeases::default());
        let repository = Arc::new(InMemoryCommunityRepository::default());
        let events = Arc::new(InMemoryCommunityEvents::default());
        let roster = Arc::new(InMemoryVoiceRoster::default());
        let presence = PresenceQueue::start(roster.clone(), events.clone(), ROSTER_TTL);
        let conference = ConferenceService::new(
            media,
            Arc::new(StaticIceServers::default()),
            leases.clone(),
            mesh,
            presence,
        );
        let communities =
            Arc::new(CommunityService::new(repository.clone(), events.clone(), roster, mesh));
        let chat = Arc::new(ChatService::new(
            conference.clone(),
            Arc::new(InMemoryChatRepository::default()),
            communities.clone(),
        ));
        let sessions = Arc::new(InMemorySessionStore::default());
        let auth = Arc::new(AuthService::new(
            Arc::new(FakeIdp),
            sessions.clone(),
            AuthSettings::new(FRONTEND, Duration::from_secs(3600), Duration::from_secs(30)),
        ));
        let lobby = lobby(&repository).await;
        let app = router(AppState {
            conference,
            chat,
            auth: auth.clone(),
            communities: communities.clone(),
            session_cookie_name: SESSION_COOKIE.into(),
            session_recheck: RECHECK,
        });

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let server =
            Self { address, auth, sessions, communities, repository, events, leases, lobby };
        server.voice_channel("general").await;
        server
    }

    /// Makes sure the lobby has a voice channel with this id (an `Auto` channel
    /// without a limit, open to every member), and returns its id.
    pub async fn voice_channel(&self, id: &str) -> ChannelId {
        let id = ChannelId::parse(id).expect("channel id");
        let state = self.repository.load(&self.lobby).await.unwrap().unwrap();
        if state.channel(&id).is_some() {
            return id;
        }
        let channel = Channel {
            id: id.clone(),
            name: Name::parse(id.as_str()).unwrap(),
            topic: Topic::default(),
            kind: ChannelKind::Voice,
            category_id: None,
            position: 100,
            synced: false,
            overrides: Vec::new(),
            media: ChannelMedia::default(),
        };
        self.repository.save_channel(&self.lobby, &channel).await.unwrap();
        id
    }

    /// Makes `user` a member of `community` through a fresh invite.
    pub async fn admit(&self, community: &CommunityId, user: &UserId, username: &str) {
        let creator = Member::new(UserId::new("sub-lobby-owner"), "lobby-owner");
        let invite = Invite::new(community.clone(), &creator, InviteSettings::default(), 0);
        self.repository.save_invite(&invite).await.unwrap();
        let member = Member::new(user.clone(), username);
        self.repository.redeem_invite(&invite.code, &member, 0).await.unwrap();
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }

    /// Logs a user in without the HTTP round trip. Returns the `Cookie` header value.
    pub async fn login(&self, username: &str, roles: &[&str]) -> String {
        let code = format!("{username}:{}", roles.join(","));
        let login =
            self.auth.complete_login(&code, None, Some("verifier.nonce")).await.expect("login");
        if !roles.is_empty() {
            self.admit(&self.lobby, &user_id(username), username).await;
        }
        format!("{SESSION_COOKIE}={}", login.session_id.as_str())
    }

    /// A WebSocket handshake for a room, optionally carrying a session cookie.
    pub fn ws_request(&self, channel: Option<&str>, cookie: Option<&str>) -> Request {
        let query = channel.map(|channel| format!("?channel_id={channel}")).unwrap_or_default();
        let mut request = format!("ws://{}/ws{query}", self.address).into_client_request().unwrap();
        if let Some(cookie) = cookie {
            request.headers_mut().insert("Cookie", HeaderValue::from_str(cookie).unwrap());
        }
        request
    }
}

/// The user id the fake identity provider gives `username`.
pub fn user_id(username: &str) -> UserId {
    UserId::new(format!("sub-{username}"))
}

/// The lobby community, owned by a user nobody logs in as.
async fn lobby(repository: &InMemoryCommunityRepository) -> CommunityId {
    let owner = User::new(user_id("lobby-owner"), "lobby-owner", None, ["member".to_owned()]);
    let name = Name::parse("Lobby").unwrap();
    let (state, member) = CommunityState::create(CommunityId::generate(), name, &owner);
    repository.create(&state, &member).await.unwrap();
    state.id().clone()
}

/// A WebRTC peer connection that speaks every default codec, like a browser's. It
/// gathers candidates on the loopback interface only: clients on one machine then
/// connect at once instead of probing every address of the host, which keeps a
/// negotiation well inside the server's timeouts and the tests' own.
pub async fn new_peer_connection() -> RTCPeerConnection {
    let mut settings = SettingEngine::default();
    settings.set_include_loopback_candidate(true);
    settings.set_network_types(vec![NetworkType::Udp4]);
    settings.set_interface_filter(Box::new(|name| name == "lo"));
    build_peer_connection(settings).await
}

async fn build_peer_connection(settings: SettingEngine) -> RTCPeerConnection {
    let mut media_engine = MediaEngine::default();
    media_engine.register_default_codecs().unwrap();
    let interceptors = register_default_interceptors(Registry::new(), &mut media_engine).unwrap();
    APIBuilder::new()
        .with_media_engine(media_engine)
        .with_interceptor_registry(interceptors)
        .with_setting_engine(settings)
        .build()
        .new_peer_connection(RTCConfiguration::default())
        .await
        .unwrap()
}

/// The next WebSocket message, parsed as JSON.
pub async fn next_json<S>(stream: &mut S) -> Value
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    let message = timeout(STEP, stream.next())
        .await
        .expect("timed out waiting for a message")
        .expect("socket closed")
        .expect("socket error");
    serde_json::from_str(message.to_text().unwrap()).unwrap()
}
