//! Time-limited TURN credentials (the "TURN REST API" scheme coturn implements as
//! `use-auth-secret`).
//!
//! The TURN server and this server share a secret. A credential is the username
//! `<expiry as Unix seconds>:<user id>` and the password
//! `base64(HMAC-SHA1(secret, username))`. The TURN server recomputes the password
//! and refuses the username once its expiry has passed, so no long-lived TURN
//! password ever reaches a browser.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, Mac};
use melior_application::IceServerProvider;
use melior_domain::{IceServer, User};
use sha1::Sha1;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// [`IceServerProvider`] that issues each participant their own expiring credential.
#[derive(Clone)]
pub struct TurnRestCredentials {
    urls: Vec<String>,
    secret: Option<String>,
    ttl: Duration,
}

impl TurnRestCredentials {
    /// Without `urls` no ICE server is handed out. Without `secret` the URLs are
    /// handed out bare, which suits STUN-only servers.
    #[must_use]
    pub fn new(urls: Vec<String>, secret: Option<String>, ttl: Duration) -> Self {
        Self { urls, secret, ttl }
    }

    fn credentials(secret: &str, user: &User, expires_at: u64) -> (String, String) {
        let username = format!("{expires_at}:{}", user.id);
        // HMAC accepts a key of any length, so this cannot fail; an empty password
        // is the fallback rather than a panic.
        let password = Hmac::<Sha1>::new_from_slice(secret.as_bytes())
            .map(|mut mac| {
                mac.update(username.as_bytes());
                STANDARD.encode(mac.finalize().into_bytes())
            })
            .unwrap_or_default();
        (username, password)
    }
}

impl IceServerProvider for TurnRestCredentials {
    fn ice_servers_for(&self, user: &User) -> Vec<IceServer> {
        if self.urls.is_empty() {
            return Vec::new();
        }
        let (username, credential) = match &self.secret {
            Some(secret) => {
                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
                let (name, password) = Self::credentials(secret, user, (now + self.ttl).as_secs());
                (Some(name), Some(password))
            }
            None => (None, None),
        };
        vec![IceServer { urls: self.urls.clone(), username, credential }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melior_domain::UserId;

    fn alice() -> User {
        User::new(UserId::new("f3c1"), "alice", None, ["member".to_owned()])
    }

    #[test]
    fn credential_matches_the_turn_rest_scheme() {
        // echo -n "1700000000:f3c1" | openssl dgst -sha1 -hmac "north" -binary | base64
        let (username, password) =
            TurnRestCredentials::credentials("north", &alice(), 1_700_000_000);
        assert_eq!(username, "1700000000:f3c1");
        assert_eq!(password, "jxw1d2Mvrx04G+VrBBZvJsirroQ=");
    }

    #[test]
    fn each_participant_gets_an_expiring_credential_of_their_own() {
        let provider = TurnRestCredentials::new(
            vec!["turn:turn.example.test:3478".into()],
            Some("north".into()),
            Duration::from_secs(600),
        );
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        let servers = provider.ice_servers_for(&alice());
        let bob = User::new(UserId::new("b0b"), "bob", None, []);

        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].urls, ["turn:turn.example.test:3478"]);
        let username = servers[0].username.clone().unwrap();
        let (expiry, id) = username.split_once(':').unwrap();
        assert_eq!(id, "f3c1");
        assert!((now + 599..=now + 601).contains(&expiry.parse::<u64>().unwrap()));
        assert_ne!(provider.ice_servers_for(&bob)[0].credential, servers[0].credential);
        assert!(!format!("{servers:?}").contains("north"), "the secret itself is never handed out");
    }

    #[test]
    fn no_urls_means_no_servers_and_no_secret_means_bare_urls() {
        let none =
            TurnRestCredentials::new(Vec::new(), Some("north".into()), Duration::from_secs(60));
        assert!(none.ice_servers_for(&alice()).is_empty());

        let stun = TurnRestCredentials::new(
            vec!["stun:stun.example.test".into()],
            None,
            Duration::from_secs(60),
        );
        assert_eq!(
            stun.ice_servers_for(&alice()),
            vec![IceServer {
                urls: vec!["stun:stun.example.test".into()],
                username: None,
                credential: None
            }]
        );
    }
}
