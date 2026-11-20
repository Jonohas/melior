//! Runtime configuration, read from environment variables and validated at startup.

use std::net::SocketAddr;
use std::time::Duration;

pub const DEFAULT_BIND: &str = "0.0.0.0:5000";
pub const DEFAULT_FRONTEND_URL: &str = "http://localhost:3000";
pub const DEFAULT_REDIS_URL: &str = "redis://127.0.0.1:6379";
pub const DEFAULT_IDP_ISSUER: &str = "http://localhost:8300/realms/melior";
pub const DEFAULT_IDP_CLIENT_ID: &str = "melior";
pub const DEFAULT_IDP_SCOPE: &str = "openid email profile";
pub const DEFAULT_SESSION_COOKIE_NAME: &str = "melior_session_id";
pub const DEFAULT_INSTANCE_ID: &str = "melior-1";
pub const DEFAULT_MESH_MAX_PARTICIPANTS: usize = 4;
/// The largest room that may run as a mesh.
pub const MESH_LIMIT_MAX: usize = 8;
/// The smallest limit that makes sense: a mesh of one is no mesh.
pub const MESH_LIMIT_MIN: usize = 2;

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum ConfigError {
    #[error("MELIOR_BIND is not a valid socket address: {0}")]
    InvalidBind(String),

    #[error("MELIOR_TURN_SECRET requires MELIOR_TURN_URLS")]
    TurnSecretWithoutUrls,

    #[error("{0} must be an http(s) URL without a path, got {1}")]
    InvalidOrigin(&'static str, String),

    #[error("{0} is not a valid number of seconds: {1}")]
    InvalidSeconds(&'static str, String),

    #[error("{0} must be at least {1} seconds")]
    TooShort(&'static str, u64),

    #[error(
        "IDP_SCOPE must contain openid and must not contain offline_access: refresh tokens are held server-side"
    )]
    InvalidScope,

    #[error("DATABASE_URL is required, e.g. postgres://user:password@127.0.0.1:5432/melior")]
    MissingDatabaseUrl,

    #[error("MELIOR_MESH_MAX_PARTICIPANTS must be 0 (mesh off) or a number from 2 to 8, got {0}")]
    InvalidMeshLimit(String),

    #[error("MELIOR_INSTANCE_ID must be 1-64 characters of A-Z, a-z, 0-9, '_' or '-'")]
    InvalidInstanceId,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Config {
    /// Address the HTTP/WebSocket server listens on (`MELIOR_BIND`).
    pub bind: SocketAddr,
    /// Names this server instance among those sharing one Redis
    /// (`MELIOR_INSTANCE_ID`). Must be unique per instance.
    pub instance_id: String,
    /// The origin browsers use for the application (`FRONTEND_URL`). The identity
    /// provider redirects back to `<FRONTEND_URL>/signin`.
    pub frontend_url: String,
    /// Sessions and room leases (`REDIS_URL`, `redis://[:password@]host:port[/db]`).
    pub redis_url: String,
    /// The room registry (`DATABASE_URL`, `postgres://user:password@host:port/db`).
    /// It carries a password, so there is no default.
    pub database_url: String,
    /// OIDC issuer (`IDP_ISSUER`), public client id (`IDP_CLIENT_ID`) and scope (`IDP_SCOPE`).
    pub idp_issuer: String,
    pub idp_client_id: String,
    pub idp_scope: String,
    /// `SESSION_COOKIE_NAME`.
    pub session_cookie_name: String,
    /// `SESSION_TTL_SECONDS`: how long a session lives without a token refresh.
    pub session_ttl: Duration,
    /// `TOKEN_REFRESH_LEAD_SECONDS`: refresh tokens this long before they expire.
    pub token_refresh_lead: Duration,
    /// `SESSION_RECHECK_SECONDS`: how often an open connection re-checks its
    /// session, its room access and this instance's lease on the room.
    pub session_recheck: Duration,
    /// ICE server URLs handed to browsers (`MELIOR_TURN_URLS`, comma separated).
    pub turn_urls: Vec<String>,
    /// The secret shared with the TURN server (`MELIOR_TURN_SECRET`), from which
    /// per-user credentials are derived. Without it the URLs are handed out bare.
    pub turn_secret: Option<String>,
    /// `MELIOR_TURN_TTL_SECONDS`: how long a TURN credential is valid.
    pub turn_ttl: Duration,
    /// The largest room that runs as a mesh (`MELIOR_MESH_MAX_PARTICIPANTS`): 0 turns
    /// mesh off, otherwise 2 to 8. See [`Config::effective_mesh_limit`].
    pub mesh_max_participants: usize,
    /// Where traces go (`OTEL_EXPORTER_OTLP_ENDPOINT`, OTLP over gRPC). Unset: no export.
    pub otlp_endpoint: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let get = |name: &str| lookup(name).map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
        let seconds = |name: &'static str, default: u64, min: u64| match get(name) {
            None => Ok(Duration::from_secs(default)),
            Some(value) => match value.parse::<u64>() {
                Ok(secs) if secs >= min => Ok(Duration::from_secs(secs)),
                Ok(_) => Err(ConfigError::TooShort(name, min)),
                Err(_) => Err(ConfigError::InvalidSeconds(name, value)),
            },
        };

        let bind = get("MELIOR_BIND").unwrap_or_else(|| DEFAULT_BIND.to_owned());
        let bind = bind.parse().map_err(|_| ConfigError::InvalidBind(bind))?;

        let instance_id =
            get("MELIOR_INSTANCE_ID").unwrap_or_else(|| DEFAULT_INSTANCE_ID.to_owned());
        let valid_instance_id = instance_id.len() <= 64
            && instance_id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
        if !valid_instance_id {
            return Err(ConfigError::InvalidInstanceId);
        }

        let frontend_url = get("FRONTEND_URL").unwrap_or_else(|| DEFAULT_FRONTEND_URL.to_owned());
        let frontend_url = frontend_url.trim_end_matches('/').to_owned();
        let is_origin = (frontend_url.starts_with("http://")
            || frontend_url.starts_with("https://"))
            && frontend_url.splitn(4, '/').count() == 3;
        if !is_origin {
            return Err(ConfigError::InvalidOrigin("FRONTEND_URL", frontend_url));
        }

        let idp_scope = get("IDP_SCOPE").unwrap_or_else(|| DEFAULT_IDP_SCOPE.to_owned());
        let scopes: Vec<&str> = idp_scope.split_whitespace().collect();
        if !scopes.contains(&"openid") || scopes.contains(&"offline_access") {
            return Err(ConfigError::InvalidScope);
        }

        let turn_urls: Vec<String> = get("MELIOR_TURN_URLS")
            .map(|urls| {
                urls.split(',')
                    .map(|url| url.trim().to_owned())
                    .filter(|url| !url.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let turn_secret = get("MELIOR_TURN_SECRET");
        if turn_secret.is_some() && turn_urls.is_empty() {
            return Err(ConfigError::TurnSecretWithoutUrls);
        }

        let mesh_max_participants = match get("MELIOR_MESH_MAX_PARTICIPANTS") {
            None => DEFAULT_MESH_MAX_PARTICIPANTS,
            Some(value) => parse_mesh_limit(&value)?,
        };

        Ok(Self {
            bind,
            instance_id,
            frontend_url,
            redis_url: get("REDIS_URL").unwrap_or_else(|| DEFAULT_REDIS_URL.to_owned()),
            database_url: get("DATABASE_URL").ok_or(ConfigError::MissingDatabaseUrl)?,
            idp_issuer: get("IDP_ISSUER").unwrap_or_else(|| DEFAULT_IDP_ISSUER.to_owned()),
            idp_client_id: get("IDP_CLIENT_ID").unwrap_or_else(|| DEFAULT_IDP_CLIENT_ID.to_owned()),
            idp_scope,
            session_cookie_name: get("SESSION_COOKIE_NAME")
                .unwrap_or_else(|| DEFAULT_SESSION_COOKIE_NAME.to_owned()),
            session_ttl: seconds("SESSION_TTL_SECONDS", 3600, 60)?,
            token_refresh_lead: seconds("TOKEN_REFRESH_LEAD_SECONDS", 30, 0)?,
            session_recheck: seconds("SESSION_RECHECK_SECONDS", 15, 1)?,
            turn_urls,
            turn_secret,
            turn_ttl: seconds("MELIOR_TURN_TTL_SECONDS", 3600, 60)?,
            mesh_max_participants,
            otlp_endpoint: get("OTEL_EXPORTER_OTLP_ENDPOINT"),
        })
    }

    /// The mesh limit the server actually applies: mesh needs TURN as its fallback
    /// path, so without `MELIOR_TURN_URLS` it is 0 (off).
    #[must_use]
    pub fn effective_mesh_limit(&self) -> usize {
        if self.turn_urls.is_empty() { 0 } else { self.mesh_max_participants }
    }

    /// How long a room stays tied to an instance that stopped renewing its lease:
    /// three missed re-checks.
    #[must_use]
    pub fn room_lease_ttl(&self) -> Duration {
        self.session_recheck * 3
    }
}

fn parse_mesh_limit(value: &str) -> Result<usize, ConfigError> {
    match value.parse::<usize>() {
        Ok(0) => Ok(0),
        Ok(limit) if (MESH_LIMIT_MIN..=MESH_LIMIT_MAX).contains(&limit) => Ok(limit),
        _ => Err(ConfigError::InvalidMeshLimit(value.to_owned())),
    }
}

/// Secrets (the database password inside the URL, the TURN secret) are left out.
impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("bind", &self.bind)
            .field("instance_id", &self.instance_id)
            .field("frontend_url", &self.frontend_url)
            .field("idp_issuer", &self.idp_issuer)
            .field("idp_client_id", &self.idp_client_id)
            .field("turn_urls", &self.turn_urls)
            .field("turn_secret", &self.turn_secret.as_ref().map(|_| "<set>"))
            .field("mesh_max_participants", &self.mesh_max_participants)
            .field("otlp_endpoint", &self.otlp_endpoint)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATABASE: (&str, &str) = ("DATABASE_URL", "postgres://melior:pw@127.0.0.1:5432/melior");

    fn config(vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
        Config::from_lookup(|name| {
            vars.iter().find(|(key, _)| *key == name).map(|(_, value)| (*value).to_owned())
        })
    }

    #[test]
    fn only_the_database_url_has_no_default() {
        assert_eq!(config(&[]), Err(ConfigError::MissingDatabaseUrl));

        let config = config(&[DATABASE]).unwrap();
        assert_eq!(config.bind, DEFAULT_BIND.parse::<SocketAddr>().unwrap());
        assert_eq!(config.instance_id, "melior-1");
        assert_eq!(config.frontend_url, DEFAULT_FRONTEND_URL);
        assert_eq!(config.redis_url, DEFAULT_REDIS_URL);
        assert_eq!(config.idp_issuer, DEFAULT_IDP_ISSUER);
        assert_eq!(config.idp_client_id, "melior");
        assert_eq!(config.session_cookie_name, "melior_session_id");
        assert_eq!(config.session_ttl, Duration::from_secs(3600));
        assert_eq!(config.token_refresh_lead, Duration::from_secs(30));
        assert_eq!(config.session_recheck, Duration::from_secs(15));
        assert_eq!(config.room_lease_ttl(), Duration::from_secs(45));
        assert!(config.turn_urls.is_empty());
        assert_eq!(config.turn_secret, None);
        assert_eq!(config.mesh_max_participants, 4);
        assert_eq!(config.otlp_endpoint, None);
    }

    #[test]
    fn reads_turn_settings() {
        let config = config(&[
            DATABASE,
            ("MELIOR_BIND", "127.0.0.1:6000"),
            ("MELIOR_TURN_URLS", "turn:a:3478?transport=udp, turn:b:3478"),
            ("MELIOR_TURN_SECRET", "north"),
            ("MELIOR_TURN_TTL_SECONDS", "600"),
        ])
        .unwrap();

        assert_eq!(config.bind, "127.0.0.1:6000".parse::<SocketAddr>().unwrap());
        assert_eq!(config.turn_urls, ["turn:a:3478?transport=udp", "turn:b:3478"]);
        assert_eq!(config.turn_secret.as_deref(), Some("north"));
        assert_eq!(config.turn_ttl, Duration::from_secs(600));
    }

    #[test]
    fn reads_and_validates_auth_settings() {
        let ok = config(&[
            DATABASE,
            ("FRONTEND_URL", "https://meet.example.test/"),
            ("SESSION_TTL_SECONDS", "120"),
            ("IDP_SCOPE", "openid profile"),
            ("MELIOR_INSTANCE_ID", "melior-2"),
            ("OTEL_EXPORTER_OTLP_ENDPOINT", "http://127.0.0.1:4317"),
        ])
        .unwrap();
        assert_eq!(ok.frontend_url, "https://meet.example.test");
        assert_eq!(ok.session_ttl, Duration::from_secs(120));
        assert_eq!(ok.instance_id, "melior-2");
        assert_eq!(ok.otlp_endpoint.as_deref(), Some("http://127.0.0.1:4317"));
    }

    #[test]
    fn rejects_values_that_cannot_work() {
        let bad = |extra: (&'static str, &'static str)| config(&[DATABASE, extra]).unwrap_err();

        assert_eq!(bad(("MELIOR_BIND", "nope")), ConfigError::InvalidBind("nope".into()));
        assert_eq!(bad(("MELIOR_TURN_SECRET", "north")), ConfigError::TurnSecretWithoutUrls);
        assert!(matches!(
            bad(("FRONTEND_URL", "meet.example.test")),
            ConfigError::InvalidOrigin("FRONTEND_URL", _)
        ));
        assert!(matches!(
            bad(("FRONTEND_URL", "https://meet.example.test/app")),
            ConfigError::InvalidOrigin("FRONTEND_URL", _)
        ));
        assert_eq!(
            bad(("SESSION_TTL_SECONDS", "soon")),
            ConfigError::InvalidSeconds("SESSION_TTL_SECONDS", "soon".into())
        );
        assert_eq!(
            bad(("SESSION_RECHECK_SECONDS", "0")),
            ConfigError::TooShort("SESSION_RECHECK_SECONDS", 1)
        );
        assert_eq!(bad(("IDP_SCOPE", "openid offline_access")), ConfigError::InvalidScope);
        assert_eq!(bad(("IDP_SCOPE", "profile")), ConfigError::InvalidScope);
        assert_eq!(bad(("MELIOR_INSTANCE_ID", "two words")), ConfigError::InvalidInstanceId);
    }

    #[test]
    fn debug_output_leaves_secrets_out() {
        let config = config(&[
            DATABASE,
            ("MELIOR_TURN_URLS", "turn:a:3478"),
            ("MELIOR_TURN_SECRET", "north-secret"),
        ])
        .unwrap();
        let printed = format!("{config:?}");
        assert!(!printed.contains("north-secret"));
        assert!(!printed.contains(":pw@"));
    }

    #[test]
    fn mesh_limit_accepts_zero_and_two_to_eight() {
        for (value, expected) in [("0", 0), ("2", 2), ("4", 4), ("8", 8)] {
            let config = config(&[DATABASE, ("MELIOR_MESH_MAX_PARTICIPANTS", value)]).unwrap();
            assert_eq!(config.mesh_max_participants, expected);
        }
    }

    #[test]
    fn mesh_limit_rejects_one_nine_negative_and_non_numeric_values() {
        for value in ["1", "9", "100", "-1", "many", "4.5"] {
            assert_eq!(
                config(&[DATABASE, ("MELIOR_MESH_MAX_PARTICIPANTS", value)]),
                Err(ConfigError::InvalidMeshLimit(value.into())),
                "{value}"
            );
        }
    }

    #[test]
    fn mesh_needs_turn_to_be_on() {
        let without = config(&[DATABASE]).unwrap();
        assert_eq!(without.mesh_max_participants, 4);
        assert_eq!(without.effective_mesh_limit(), 0, "no TURN: mesh is off, startup still works");

        let with = config(&[DATABASE, ("MELIOR_TURN_URLS", "turn:a:3478")]).unwrap();
        assert_eq!(with.effective_mesh_limit(), 4);

        let off = config(&[
            DATABASE,
            ("MELIOR_TURN_URLS", "turn:a:3478"),
            ("MELIOR_MESH_MAX_PARTICIPANTS", "0"),
        ])
        .unwrap();
        assert_eq!(off.effective_mesh_limit(), 0);
    }
}
