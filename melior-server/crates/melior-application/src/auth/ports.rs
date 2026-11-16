//! What the login use cases need from the outside world.

use async_trait::async_trait;
use melior_domain::{AuthSessionId, User};
use std::time::Duration;

/// The identity provider's tokens for one login. Opaque to the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Present after a login; a refresh may omit it.
    pub id_token: Option<String>,
    /// When the access token stops being valid, in seconds since the Unix epoch.
    pub access_token_expires_at: u64,
}

/// A user the identity provider has vouched for, with the tokens that prove it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authentication {
    pub user: User,
    pub tokens: Tokens,
}

/// Where to send the browser to log in, and the secrets that must come back with
/// the callback for it to be accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationRequest {
    pub url: String,
    /// PKCE code verifier: proves the callback belongs to the browser that started.
    pub code_verifier: String,
    /// Binds the id token to this login.
    pub nonce: String,
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum IdpError {
    /// The grant (authorization code or refresh token) is not valid any more.
    #[error("the identity provider rejected the grant")]
    InvalidGrant,

    /// The identity provider answered, but with something that cannot be trusted.
    #[error("invalid response from the identity provider: {0}")]
    InvalidResponse(String),

    #[error("identity provider unavailable: {0}")]
    Unavailable(String),
}

/// An OpenID Connect identity provider (authorization code flow with PKCE).
#[async_trait]
pub trait IdentityProvider: Send + Sync {
    async fn authorization_request(
        &self,
        redirect_uri: &str,
        state: &str,
    ) -> Result<AuthorizationRequest, IdpError>;

    /// Redeems an authorization code and verifies what comes back.
    async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
        nonce: &str,
        redirect_uri: &str,
    ) -> Result<Authentication, IdpError>;

    async fn refresh(&self, refresh_token: &str) -> Result<Authentication, IdpError>;

    /// The URL that also ends the user's session at the identity provider.
    async fn logout_url(
        &self,
        id_token: &str,
        post_logout_redirect_uri: &str,
    ) -> Result<String, IdpError>;
}

/// What is kept server-side for a logged-in browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSession {
    pub user: User,
    pub tokens: Tokens,
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[error("session store: {0}")]
pub struct StoreError(pub String);

/// Server-side session storage with expiry.
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// Stores (or replaces) a session that expires after `ttl`.
    async fn save(
        &self,
        id: &AuthSessionId,
        session: &StoredSession,
        ttl: Duration,
    ) -> Result<(), StoreError>;

    /// Loads a session. Reading does not extend its lifetime.
    async fn load(&self, id: &AuthSessionId) -> Result<Option<StoredSession>, StoreError>;

    async fn delete(&self, id: &AuthSessionId) -> Result<(), StoreError>;

    /// Claims the right to refresh a session's tokens for at most `ttl`. A refresh
    /// token can be redeemed once, so with several server instances exactly one
    /// may try. Returns a lease to hand back to [`Self::release_refresh`], or
    /// `None` if someone else holds the claim.
    async fn claim_refresh(
        &self,
        id: &AuthSessionId,
        ttl: Duration,
    ) -> Result<Option<String>, StoreError>;

    /// Gives a refresh claim back. A lease that has expired or was never issued
    /// releases nothing.
    async fn release_refresh(&self, id: &AuthSessionId, lease: &str) -> Result<(), StoreError>;
}
