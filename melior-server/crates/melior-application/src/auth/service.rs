use super::ports::{IdentityProvider, IdpError, SessionStore, StoreError, StoredSession};
use melior_domain::{AuthSessionId, ReturnPath, User};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex as AsyncMutex;
use tracing::{info, warn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSettings {
    /// The origin browsers use for the application, e.g. `http://localhost:3000`.
    /// The login callback and every post-login redirect are built from it, never
    /// from request headers.
    pub frontend_url: String,
    /// How long a session lives without a successful token refresh.
    pub session_ttl: Duration,
    /// Tokens are refreshed when they are this close to expiring.
    pub refresh_lead: Duration,
}

impl AuthSettings {
    #[must_use]
    pub fn new(frontend_url: &str, session_ttl: Duration, refresh_lead: Duration) -> Self {
        Self {
            frontend_url: frontend_url.trim_end_matches('/').to_owned(),
            session_ttl,
            refresh_lead,
        }
    }

    /// An absolute URL inside the application.
    #[must_use]
    pub fn frontend(&self, path: &str) -> String {
        format!("{}{}", self.frontend_url, path)
    }

    /// Whether cookies must be marked `Secure`.
    #[must_use]
    pub fn is_https(&self) -> bool {
        self.frontend_url.starts_with("https://")
    }

    fn callback_url(&self) -> String {
        self.frontend("/signin")
    }
}

#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    /// The callback arrived without the state this browser was given at `/login`:
    /// it was started elsewhere, or it is older than the state's lifetime.
    #[error("login state is missing or malformed")]
    MissingLoginState,

    #[error(transparent)]
    Idp(#[from] IdpError),

    #[error(transparent)]
    Store(#[from] StoreError),
}

/// The first half of a login: where to send the browser, and what it must bring back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginRedirect {
    pub location: String,
    /// Opaque; to be held by the browser until the callback (in a short-lived cookie).
    pub login_state: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedLogin {
    pub session_id: AuthSessionId,
    /// Where the user wanted to go, as an absolute URL inside the application.
    pub location: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSession {
    pub user: User,
    /// The tokens were refreshed and the session's lifetime restarted, so the
    /// session cookie should be issued again.
    pub renewed: bool,
}

/// What a session cookie resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Active(ActiveSession),
    /// There is no such session (any more): never logged in, logged out, expired,
    /// or ended by the identity provider.
    Ended,
    /// The session exists but its tokens could not be refreshed right now, because
    /// the identity provider did not answer or another instance is still at it.
    Unverifiable,
}

impl SessionState {
    fn active(user: User, renewed: bool) -> Self {
        Self::Active(ActiveSession { user, renewed })
    }
}

/// How long one instance may hold the right to refresh a session. Longer than any
/// refresh takes; short enough that a crashed instance does not block the session.
const REFRESH_CLAIM_TTL: Duration = Duration::from_secs(20);
/// How long to wait for a refresh another instance is doing, and how often to look.
const REFRESH_WAIT: Duration = Duration::from_secs(3);
const REFRESH_POLL: Duration = Duration::from_millis(100);

/// The login use cases.
pub struct AuthService {
    idp: Arc<dyn IdentityProvider>,
    sessions: Arc<dyn SessionStore>,
    settings: AuthSettings,
    /// A refresh token can be redeemed only once, so two requests racing to refresh
    /// the same session would invalidate each other. This serialises them within
    /// the process; the session store's refresh claim does it across instances.
    refreshing: AsyncMutex<()>,
}

impl AuthService {
    pub fn new(
        idp: Arc<dyn IdentityProvider>,
        sessions: Arc<dyn SessionStore>,
        settings: AuthSettings,
    ) -> Self {
        Self { idp, sessions, settings, refreshing: AsyncMutex::new(()) }
    }

    pub fn settings(&self) -> &AuthSettings {
        &self.settings
    }

    /// Starts a login that will return the user to `redirect_to` (an app-relative
    /// path; anything else is replaced by the application root).
    #[tracing::instrument(name = "auth.begin_login", skip_all)]
    pub async fn begin_login(&self, redirect_to: Option<&str>) -> Result<LoginRedirect, AuthError> {
        let return_path = ReturnPath::parse(redirect_to.unwrap_or("/"));
        let request = self
            .idp
            .authorization_request(&self.settings.callback_url(), return_path.as_str())
            .await?;
        Ok(LoginRedirect {
            location: request.url,
            login_state: format!("{}.{}", request.code_verifier, request.nonce),
        })
    }

    /// Finishes a login from the identity provider's callback and opens a session.
    #[tracing::instrument(name = "auth.complete_login", skip_all)]
    pub async fn complete_login(
        &self,
        code: &str,
        state: Option<&str>,
        login_state: Option<&str>,
    ) -> Result<CompletedLogin, AuthError> {
        let (code_verifier, nonce) = login_state
            .and_then(|value| value.split_once('.'))
            .filter(|(verifier, nonce)| !verifier.is_empty() && !nonce.is_empty())
            .ok_or(AuthError::MissingLoginState)?;

        let authentication = self
            .idp
            .exchange_code(code, code_verifier, nonce, &self.settings.callback_url())
            .await?;

        let session_id = AuthSessionId::generate();
        let session = StoredSession { user: authentication.user, tokens: authentication.tokens };
        self.sessions.save(&session_id, &session, self.settings.session_ttl).await?;
        info!(user = %session.user.username, "user logged in");

        // `state` came back through the browser: validate it again.
        let return_path = ReturnPath::parse(state.unwrap_or("/"));
        Ok(CompletedLogin { session_id, location: self.settings.frontend(return_path.as_str()) })
    }

    /// Resolves a session cookie to its user, refreshing the tokens when they are
    /// about to expire. `None` means "not logged in" for whatever reason.
    pub async fn session(&self, id: &AuthSessionId) -> Result<Option<ActiveSession>, AuthError> {
        Ok(match self.check(id).await? {
            SessionState::Active(active) => Some(active),
            SessionState::Ended | SessionState::Unverifiable => None,
        })
    }

    /// Like [`Self::session`], but tells "the session is over" apart from "it could
    /// not be verified just now". Long-lived connections use this to decide whether
    /// to hang up: only a session that is definitely over justifies it.
    pub async fn check(&self, id: &AuthSessionId) -> Result<SessionState, AuthError> {
        let Some(session) = self.sessions.load(id).await? else {
            return Ok(SessionState::Ended);
        };
        if !self.needs_refresh(&session) {
            return Ok(SessionState::active(session.user, false));
        }

        // One refresh at a time in this process...
        let _one_at_a_time = self.refreshing.lock().await;
        // Another request may have refreshed (or ended) the session meanwhile.
        let Some(session) = self.sessions.load(id).await? else {
            return Ok(SessionState::Ended);
        };
        if !self.needs_refresh(&session) {
            return Ok(SessionState::active(session.user, false));
        }

        // ...and one across all server instances.
        let Some(lease) = self.sessions.claim_refresh(id, REFRESH_CLAIM_TTL).await? else {
            return self.await_refresh_elsewhere(id).await;
        };
        let outcome = self.refresh(id, session).await;
        if let Err(error) = self.sessions.release_refresh(id, &lease).await {
            warn!(%error, "could not release the refresh claim");
        }
        outcome
    }

    /// Redeems the session's refresh token and stores what comes back.
    #[tracing::instrument(name = "auth.refresh", skip_all, fields(user = %session.user.username))]
    async fn refresh(
        &self,
        id: &AuthSessionId,
        session: StoredSession,
    ) -> Result<SessionState, AuthError> {
        let Some(refresh_token) = session.tokens.refresh_token.as_deref() else {
            self.sessions.delete(id).await?;
            return Ok(SessionState::Ended);
        };
        match self.idp.refresh(refresh_token).await {
            Ok(authentication) => {
                let mut tokens = authentication.tokens;
                // A refresh may omit what has not changed.
                tokens.id_token = tokens.id_token.or(session.tokens.id_token);
                tokens.refresh_token = tokens.refresh_token.or(session.tokens.refresh_token);
                let renewed = StoredSession { user: authentication.user, tokens };
                self.sessions.save(id, &renewed, self.settings.session_ttl).await?;
                Ok(SessionState::active(renewed.user, true))
            }
            // The identity provider ended the login (logout, disabled user, expiry).
            Err(IdpError::InvalidGrant) => {
                info!(user = %session.user.username, "session ended by the identity provider");
                self.sessions.delete(id).await?;
                Ok(SessionState::Ended)
            }
            // Could not ask. The session stays so that a later request can try again.
            Err(error) => {
                warn!(user = %session.user.username, %error, "token refresh failed");
                Ok(SessionState::Unverifiable)
            }
        }
    }

    /// Another instance is refreshing this session: waits for the result to appear
    /// in the store instead of redeeming the same refresh token a second time.
    async fn await_refresh_elsewhere(&self, id: &AuthSessionId) -> Result<SessionState, AuthError> {
        let deadline = tokio::time::Instant::now() + REFRESH_WAIT;
        while tokio::time::Instant::now() < deadline {
            tokio::time::sleep(REFRESH_POLL).await;
            match self.sessions.load(id).await? {
                None => return Ok(SessionState::Ended),
                Some(session) if !self.needs_refresh(&session) => {
                    return Ok(SessionState::active(session.user, true));
                }
                Some(_) => {}
            }
        }
        Ok(SessionState::Unverifiable)
    }

    /// Ends a session. Returns where to send the browser: the identity provider's
    /// logout when possible, so the single sign-on session ends too, otherwise the
    /// application root.
    #[tracing::instrument(name = "auth.logout", skip_all)]
    pub async fn logout(&self, id: Option<&AuthSessionId>) -> String {
        let home = self.settings.frontend("/");
        let Some(id) = id else {
            return home;
        };

        let session = match self.sessions.load(id).await {
            Ok(session) => session,
            Err(error) => {
                warn!(%error, "could not load the session being logged out");
                None
            }
        };
        if let Err(error) = self.sessions.delete(id).await {
            warn!(%error, "could not delete the session being logged out");
        }

        let Some(session) = session else {
            return home;
        };
        info!(user = %session.user.username, "user logged out");
        let Some(id_token) = session.tokens.id_token else {
            return home;
        };
        match self.idp.logout_url(&id_token, &home).await {
            Ok(url) => url,
            Err(error) => {
                warn!(%error, "identity provider logout unavailable");
                home
            }
        }
    }

    fn needs_refresh(&self, session: &StoredSession) -> bool {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        now + self.settings.refresh_lead.as_secs() >= session.tokens.access_token_expires_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::InMemorySessionStore;
    use crate::auth::ports::{Authentication, AuthorizationRequest, Tokens};
    use async_trait::async_trait;
    use melior_domain::UserId;
    use std::sync::Mutex;

    fn now() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    }

    fn alice(roles: &[&str]) -> User {
        User::new(
            UserId::new("sub-alice"),
            "alice",
            Some("alice@example.test".into()),
            roles.iter().copied().map(str::to_owned),
        )
    }

    /// An identity provider that issues access tokens with a configurable lifetime.
    struct FakeIdp {
        access_token_lifetime: Mutex<i64>,
        refresh_result: Mutex<Option<IdpError>>,
        refreshes: Mutex<Vec<String>>,
        exchanged: Mutex<Vec<(String, String, String, String)>>,
    }

    impl FakeIdp {
        fn new() -> Self {
            Self {
                access_token_lifetime: Mutex::new(300),
                refresh_result: Mutex::new(None),
                refreshes: Mutex::new(Vec::new()),
                exchanged: Mutex::new(Vec::new()),
            }
        }

        fn authentication(&self, generation: usize, roles: &[&str]) -> Authentication {
            let lifetime = *self.access_token_lifetime.lock().unwrap();
            Authentication {
                user: alice(roles),
                tokens: Tokens {
                    access_token: format!("access-{generation}"),
                    refresh_token: Some(format!("refresh-{generation}")),
                    id_token: (generation == 0).then(|| "id-token".to_owned()),
                    access_token_expires_at: now().saturating_add_signed(lifetime),
                },
            }
        }
    }

    #[async_trait]
    impl IdentityProvider for FakeIdp {
        async fn authorization_request(
            &self,
            redirect_uri: &str,
            state: &str,
        ) -> Result<AuthorizationRequest, IdpError> {
            Ok(AuthorizationRequest {
                url: format!("https://idp.test/auth?redirect_uri={redirect_uri}&state={state}"),
                code_verifier: "verifier".into(),
                nonce: "nonce".into(),
            })
        }

        async fn exchange_code(
            &self,
            code: &str,
            code_verifier: &str,
            nonce: &str,
            redirect_uri: &str,
        ) -> Result<Authentication, IdpError> {
            if code != "good-code" {
                return Err(IdpError::InvalidGrant);
            }
            self.exchanged.lock().unwrap().push((
                code.into(),
                code_verifier.into(),
                nonce.into(),
                redirect_uri.into(),
            ));
            Ok(self.authentication(0, &["member"]))
        }

        async fn refresh(&self, refresh_token: &str) -> Result<Authentication, IdpError> {
            let mut refreshes = self.refreshes.lock().unwrap();
            refreshes.push(refresh_token.to_owned());
            if let Some(error) = self.refresh_result.lock().unwrap().clone() {
                return Err(error);
            }
            // Roles can change between refreshes.
            Ok(self.authentication(refreshes.len(), &["member", "admin"]))
        }

        async fn logout_url(
            &self,
            id_token: &str,
            post_logout_redirect_uri: &str,
        ) -> Result<String, IdpError> {
            Ok(format!("https://idp.test/logout?hint={id_token}&to={post_logout_redirect_uri}"))
        }
    }

    struct Harness {
        idp: Arc<FakeIdp>,
        store: Arc<InMemorySessionStore>,
        service: AuthService,
    }

    impl Harness {
        fn new() -> Self {
            let idp = Arc::new(FakeIdp::new());
            let store = Arc::new(InMemorySessionStore::default());
            let service = AuthService::new(
                idp.clone(),
                store.clone(),
                AuthSettings::new(
                    "http://localhost:3000/",
                    Duration::from_secs(3600),
                    Duration::from_secs(30),
                ),
            );
            Self { idp, store, service }
        }

        async fn login(&self) -> AuthSessionId {
            self.service
                .complete_login("good-code", Some("/"), Some("verifier.nonce"))
                .await
                .unwrap()
                .session_id
        }
    }

    #[tokio::test]
    async fn login_redirects_to_the_idp_and_returns_to_the_requested_path() {
        let h = Harness::new();

        let start = h.service.begin_login(Some("/rooms/standup")).await.unwrap();
        assert_eq!(
            start.location,
            "https://idp.test/auth?redirect_uri=http://localhost:3000/signin&state=/rooms/standup"
        );
        assert_eq!(start.login_state, "verifier.nonce");

        let done = h
            .service
            .complete_login("good-code", Some("/rooms/standup"), Some(&start.login_state))
            .await
            .unwrap();
        assert_eq!(done.location, "http://localhost:3000/rooms/standup");
        assert_eq!(
            h.idp.exchanged.lock().unwrap().as_slice(),
            [(
                "good-code".to_owned(),
                "verifier".to_owned(),
                "nonce".to_owned(),
                "http://localhost:3000/signin".to_owned()
            )]
        );

        let active = h.service.session(&done.session_id).await.unwrap().unwrap();
        assert_eq!(active.user, alice(&["member"]));
        assert!(!active.renewed);
    }

    #[tokio::test]
    async fn unsafe_return_paths_fall_back_to_the_root() {
        let h = Harness::new();

        let start = h.service.begin_login(Some("https://evil.example/")).await.unwrap();
        assert!(start.location.ends_with("&state=/"));

        let done = h
            .service
            .complete_login("good-code", Some("//evil.example"), Some("verifier.nonce"))
            .await
            .unwrap();
        assert_eq!(done.location, "http://localhost:3000/");
    }

    #[tokio::test]
    async fn a_callback_without_login_state_or_with_a_bad_code_opens_no_session() {
        let h = Harness::new();

        for login_state in [None, Some(""), Some("no-separator"), Some(".nonce"), Some("verifier.")]
        {
            assert!(matches!(
                h.service.complete_login("good-code", Some("/"), login_state).await,
                Err(AuthError::MissingLoginState)
            ));
        }
        assert!(matches!(
            h.service.complete_login("stolen-code", Some("/"), Some("verifier.nonce")).await,
            Err(AuthError::Idp(IdpError::InvalidGrant))
        ));
        assert_eq!(h.store.len(), 0);
    }

    #[tokio::test]
    async fn unknown_sessions_are_not_logged_in() {
        let h = Harness::new();
        assert_eq!(h.service.session(&AuthSessionId::generate()).await.unwrap(), None);
    }

    #[tokio::test]
    async fn expiring_tokens_are_refreshed_once_and_the_session_renewed() {
        let h = Harness::new();
        *h.idp.access_token_lifetime.lock().unwrap() = 10; // inside the 30 s lead
        let id = h.login().await;
        *h.idp.access_token_lifetime.lock().unwrap() = 300;

        let active = h.service.session(&id).await.unwrap().unwrap();
        assert!(active.renewed);
        assert_eq!(active.user, alice(&["admin", "member"]), "roles follow the new token");
        assert_eq!(h.idp.refreshes.lock().unwrap().as_slice(), ["refresh-0"]);

        let stored = h.store.load(&id).await.unwrap().unwrap();
        assert_eq!(stored.tokens.access_token, "access-1");
        assert_eq!(stored.tokens.refresh_token.as_deref(), Some("refresh-1"));
        assert_eq!(
            stored.tokens.id_token.as_deref(),
            Some("id-token"),
            "kept when the refresh response omits it"
        );

        let again = h.service.session(&id).await.unwrap().unwrap();
        assert!(!again.renewed);
        assert_eq!(h.idp.refreshes.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_rejected_refresh_ends_the_session() {
        let h = Harness::new();
        *h.idp.access_token_lifetime.lock().unwrap() = 0;
        let id = h.login().await;
        *h.idp.refresh_result.lock().unwrap() = Some(IdpError::InvalidGrant);

        assert_eq!(h.service.session(&id).await.unwrap(), None);
        assert_eq!(h.store.len(), 0);
    }

    #[tokio::test]
    async fn an_unreachable_idp_fails_the_request_but_keeps_the_session() {
        let h = Harness::new();
        *h.idp.access_token_lifetime.lock().unwrap() = 0;
        let id = h.login().await;
        *h.idp.refresh_result.lock().unwrap() = Some(IdpError::Unavailable("down".into()));

        assert_eq!(h.service.session(&id).await.unwrap(), None);
        assert_eq!(h.store.len(), 1);

        *h.idp.refresh_result.lock().unwrap() = None;
        *h.idp.access_token_lifetime.lock().unwrap() = 300;
        assert!(h.service.session(&id).await.unwrap().unwrap().renewed);
    }

    #[tokio::test]
    async fn check_tells_an_ended_session_from_an_unverifiable_one() {
        let h = Harness::new();
        assert_eq!(h.service.check(&AuthSessionId::generate()).await.unwrap(), SessionState::Ended);

        *h.idp.access_token_lifetime.lock().unwrap() = 0;
        let id = h.login().await;
        *h.idp.refresh_result.lock().unwrap() = Some(IdpError::Unavailable("down".into()));
        assert_eq!(h.service.check(&id).await.unwrap(), SessionState::Unverifiable);

        *h.idp.refresh_result.lock().unwrap() = Some(IdpError::InvalidGrant);
        assert_eq!(h.service.check(&id).await.unwrap(), SessionState::Ended);
    }

    #[tokio::test(start_paused = true)]
    async fn a_refresh_claimed_by_another_instance_is_awaited_not_repeated() {
        let h = Harness::new();
        *h.idp.access_token_lifetime.lock().unwrap() = 0;
        let id = h.login().await;
        *h.idp.access_token_lifetime.lock().unwrap() = 300;

        // Another instance holds the claim and finishes its refresh a moment later.
        let lease = h.store.claim_refresh(&id, Duration::from_secs(20)).await.unwrap().unwrap();
        let store = h.store.clone();
        let refreshed =
            StoredSession { user: alice(&["member"]), tokens: h.idp.authentication(7, &[]).tokens };
        let other_instance = {
            let id = id.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(450)).await;
                store.save(&id, &refreshed, Duration::from_secs(3600)).await.unwrap();
                store.release_refresh(&id, &lease).await.unwrap();
            })
        };

        let state = h.service.check(&id).await.unwrap();
        other_instance.await.unwrap();

        assert!(matches!(state, SessionState::Active(ActiveSession { renewed: true, .. })));
        assert!(
            h.idp.refreshes.lock().unwrap().is_empty(),
            "this instance must not redeem the token"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_refresh_that_never_finishes_elsewhere_leaves_the_session_unverifiable() {
        let h = Harness::new();
        *h.idp.access_token_lifetime.lock().unwrap() = 0;
        let id = h.login().await;
        h.store.claim_refresh(&id, Duration::from_secs(20)).await.unwrap().unwrap();

        assert_eq!(h.service.check(&id).await.unwrap(), SessionState::Unverifiable);
        assert_eq!(h.store.len(), 1, "the session is kept");
        assert!(h.idp.refreshes.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn logout_deletes_the_session_and_goes_through_the_idp() {
        let h = Harness::new();
        let id = h.login().await;

        assert_eq!(
            h.service.logout(Some(&id)).await,
            "https://idp.test/logout?hint=id-token&to=http://localhost:3000/"
        );
        assert_eq!(h.store.len(), 0);
        assert_eq!(h.service.session(&id).await.unwrap(), None);

        // Without a session there is nothing to end at the identity provider.
        assert_eq!(h.service.logout(Some(&id)).await, "http://localhost:3000/");
        assert_eq!(h.service.logout(None).await, "http://localhost:3000/");
    }
}
