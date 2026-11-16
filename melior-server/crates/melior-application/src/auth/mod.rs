//! Login use cases: the server acts as a backend-for-frontend, so tokens never reach
//! the browser. It runs the OIDC code flow, keeps the tokens in a session store and
//! hands the browser only an opaque session cookie.

mod memory;
mod ports;
mod service;

pub use memory::InMemorySessionStore;
pub use ports::{
    Authentication, AuthorizationRequest, IdentityProvider, IdpError, SessionStore, StoreError,
    StoredSession, Tokens,
};
pub use service::{
    ActiveSession, AuthError, AuthService, AuthSettings, CompletedLogin, LoginRedirect,
    SessionState,
};
