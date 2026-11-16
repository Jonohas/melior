//! OpenID Connect identity provider adapter (developed against Keycloak).

mod claims;
mod provider;

pub use provider::{OidcIdentityProvider, OidcSettings};
