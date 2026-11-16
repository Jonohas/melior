use super::claims::{AccessTokenClaims, IdTokenClaims};
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use melior_application::{
    Authentication, AuthorizationRequest, IdentityProvider, IdpError, Tokens,
};
use rand::RngCore;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use url::Url;

/// Signature algorithms accepted on tokens: asymmetric only, so a token can never
/// be "verified" with something the client itself knows.
const ALGORITHMS: [Algorithm; 8] = [
    Algorithm::RS256,
    Algorithm::RS384,
    Algorithm::RS512,
    Algorithm::PS256,
    Algorithm::PS384,
    Algorithm::PS512,
    Algorithm::ES256,
    Algorithm::ES384,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcSettings {
    /// The issuer URL, e.g. `http://localhost:8300/realms/melior`. Endpoints and
    /// signing keys are discovered from it.
    pub issuer: String,
    /// A public client: the code exchange is protected by PKCE, not a secret.
    pub client_id: String,
    pub scope: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Metadata {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    end_session_endpoint: Option<String>,
    jwks_uri: String,
}

struct Discovered {
    metadata: Metadata,
    keys: JwkSet,
}

// The field names are the token endpoint's (RFC 6749, OIDC Core).
#[allow(clippy::struct_field_names)]
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    id_token: Option<String>,
}

#[derive(Deserialize)]
struct TokenErrorResponse {
    error: Option<String>,
}

/// [`IdentityProvider`] speaking OpenID Connect: authorization code flow with PKCE
/// (S256) for a public client. The provider's metadata and keys are fetched on first
/// use, so the server can start before the identity provider is reachable.
pub struct OidcIdentityProvider {
    http: reqwest::Client,
    settings: OidcSettings,
    discovered: RwLock<Option<Arc<Discovered>>>,
}

impl OidcIdentityProvider {
    pub fn new(settings: OidcSettings) -> Result<Self, IdpError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(unavailable)?;
        Ok(Self {
            http,
            settings: OidcSettings {
                issuer: settings.issuer.trim_end_matches('/').to_owned(),
                ..settings
            },
            discovered: RwLock::new(None),
        })
    }

    async fn discovered(&self) -> Result<Arc<Discovered>, IdpError> {
        if let Some(discovered) = self.discovered.read().await.as_ref() {
            return Ok(discovered.clone());
        }
        self.discover().await
    }

    /// Fetches metadata and signing keys, replacing what is cached.
    async fn discover(&self) -> Result<Arc<Discovered>, IdpError> {
        let mut slot = self.discovered.write().await;

        let url = format!("{}/.well-known/openid-configuration", self.settings.issuer);
        let metadata: Metadata = self.get_json(&url).await?;
        if metadata.issuer.trim_end_matches('/') != self.settings.issuer {
            return Err(IdpError::InvalidResponse(format!(
                "metadata is for issuer {}, expected {}",
                metadata.issuer, self.settings.issuer
            )));
        }
        let keys: JwkSet = self.get_json(&metadata.jwks_uri).await?;

        let discovered = Arc::new(Discovered { metadata, keys });
        *slot = Some(discovered.clone());
        Ok(discovered)
    }

    async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, IdpError> {
        let response = self.http.get(url).send().await.map_err(unavailable)?;
        if !response.status().is_success() {
            return Err(IdpError::Unavailable(format!("GET {url} returned {}", response.status())));
        }
        response.json().await.map_err(invalid)
    }

    async fn token_request(&self, form: &[(&str, &str)]) -> Result<TokenResponse, IdpError> {
        let discovered = self.discovered().await?;
        let response = self
            .http
            .post(&discovered.metadata.token_endpoint)
            .form(form)
            .send()
            .await
            .map_err(unavailable)?;

        let status = response.status();
        if status.is_success() {
            return response.json().await.map_err(invalid);
        }
        let error = response.json::<TokenErrorResponse>().await.ok().and_then(|body| body.error);
        match error.as_deref() {
            Some("invalid_grant") => Err(IdpError::InvalidGrant),
            _ if status.is_server_error() => {
                Err(IdpError::Unavailable(format!("token endpoint returned {status}")))
            }
            other => Err(IdpError::InvalidResponse(format!(
                "token endpoint returned {status} ({})",
                other.unwrap_or("no error code")
            ))),
        }
    }

    /// Verifies a token's signature, issuer, expiry and (if given) audience.
    /// If the signing key is unknown the keys are fetched again once, which covers
    /// key rotation at the identity provider.
    async fn verify<C: DeserializeOwned>(
        &self,
        token: &str,
        audience: Option<&str>,
    ) -> Result<C, IdpError> {
        let discovered = self.discovered().await?;
        match verify_with(token, &discovered.keys, &self.settings.issuer, audience) {
            Err(VerifyError::UnknownKey) => {
                let discovered = self.discover().await?;
                verify_with(token, &discovered.keys, &self.settings.issuer, audience)
                    .map_err(IdpError::from)
            }
            other => other.map_err(IdpError::from),
        }
    }

    /// Turns a token response into an authentication, trusting nothing unverified.
    async fn authenticate(
        &self,
        response: TokenResponse,
        expected_nonce: Option<&str>,
    ) -> Result<Authentication, IdpError> {
        let access: AccessTokenClaims = self.verify(&response.access_token, None).await?;

        if let Some(id_token) = &response.id_token {
            let id: IdTokenClaims = self.verify(id_token, Some(&self.settings.client_id)).await?;
            if id.sub != access.sub {
                return Err(IdpError::InvalidResponse(
                    "id token and access token name different subjects".into(),
                ));
            }
            if let Some(expected) = expected_nonce
                && id.nonce.as_deref() != Some(expected)
            {
                return Err(IdpError::InvalidResponse(
                    "id token nonce does not match this login".into(),
                ));
            }
        } else if expected_nonce.is_some() {
            return Err(IdpError::InvalidResponse("no id token in response".into()));
        }

        let access_token_expires_at = access.exp;
        Ok(Authentication {
            user: access.into_user(&self.settings.client_id),
            tokens: Tokens {
                access_token: response.access_token,
                refresh_token: response.refresh_token,
                id_token: response.id_token,
                access_token_expires_at,
            },
        })
    }
}

#[async_trait]
impl IdentityProvider for OidcIdentityProvider {
    async fn authorization_request(
        &self,
        redirect_uri: &str,
        state: &str,
    ) -> Result<AuthorizationRequest, IdpError> {
        let discovered = self.discovered().await?;
        let code_verifier = random_token(32);
        let nonce = random_token(16);
        let url = authorization_url(
            &discovered.metadata.authorization_endpoint,
            &self.settings,
            redirect_uri,
            state,
            &code_challenge(&code_verifier),
            &nonce,
        )?;
        Ok(AuthorizationRequest { url, code_verifier, nonce })
    }

    async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
        nonce: &str,
        redirect_uri: &str,
    ) -> Result<Authentication, IdpError> {
        let response = self
            .token_request(&[
                ("grant_type", "authorization_code"),
                ("client_id", &self.settings.client_id),
                ("code", code),
                ("redirect_uri", redirect_uri),
                ("code_verifier", code_verifier),
            ])
            .await?;
        self.authenticate(response, Some(nonce)).await
    }

    async fn refresh(&self, refresh_token: &str) -> Result<Authentication, IdpError> {
        let response = self
            .token_request(&[
                ("grant_type", "refresh_token"),
                ("client_id", &self.settings.client_id),
                ("refresh_token", refresh_token),
            ])
            .await?;
        self.authenticate(response, None).await
    }

    async fn logout_url(
        &self,
        id_token: &str,
        post_logout_redirect_uri: &str,
    ) -> Result<String, IdpError> {
        let discovered = self.discovered().await?;
        let endpoint = discovered
            .metadata
            .end_session_endpoint
            .as_deref()
            .ok_or_else(|| IdpError::InvalidResponse("no end_session_endpoint".into()))?;
        let mut url = Url::parse(endpoint).map_err(invalid)?;
        url.query_pairs_mut()
            .append_pair("id_token_hint", id_token)
            .append_pair("post_logout_redirect_uri", post_logout_redirect_uri);
        Ok(url.into())
    }
}

fn authorization_url(
    endpoint: &str,
    settings: &OidcSettings,
    redirect_uri: &str,
    state: &str,
    code_challenge: &str,
    nonce: &str,
) -> Result<String, IdpError> {
    let mut url = Url::parse(endpoint).map_err(invalid)?;
    url.query_pairs_mut()
        .append_pair("client_id", &settings.client_id)
        .append_pair("response_type", "code")
        .append_pair("scope", &settings.scope)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("state", state)
        .append_pair("nonce", nonce)
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(url.into())
}

/// `len` bytes from the operating system's random source, base64url encoded.
fn random_token(len: usize) -> String {
    let mut bytes = vec![0u8; len];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// The PKCE S256 challenge for a verifier (RFC 7636).
fn code_challenge(code_verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(code_verifier.as_bytes()))
}

#[derive(Debug)]
enum VerifyError {
    /// The token names a key that is not in the key set (yet).
    UnknownKey,
    Invalid(String),
}

impl From<VerifyError> for IdpError {
    fn from(error: VerifyError) -> Self {
        match error {
            VerifyError::UnknownKey => {
                IdpError::InvalidResponse("token signed with an unknown key".into())
            }
            VerifyError::Invalid(reason) => IdpError::InvalidResponse(reason),
        }
    }
}

fn verify_with<C: DeserializeOwned>(
    token: &str,
    keys: &JwkSet,
    issuer: &str,
    audience: Option<&str>,
) -> Result<C, VerifyError> {
    let header = decode_header(token).map_err(|e| VerifyError::Invalid(e.to_string()))?;
    if !ALGORITHMS.contains(&header.alg) {
        return Err(VerifyError::Invalid(format!(
            "unacceptable signature algorithm {:?}",
            header.alg
        )));
    }
    let key_id = header.kid.ok_or_else(|| VerifyError::Invalid("token has no key id".into()))?;
    let jwk = keys.find(&key_id).ok_or(VerifyError::UnknownKey)?;
    let key = DecodingKey::from_jwk(jwk).map_err(|e| VerifyError::Invalid(e.to_string()))?;

    let mut validation = Validation::new(header.alg);
    validation.set_issuer(&[issuer]);
    validation.set_required_spec_claims(&["exp", "iss", "sub"]);
    match audience {
        Some(audience) => validation.set_audience(&[audience]),
        None => validation.validate_aud = false,
    }
    decode::<C>(token, &key, &validation)
        .map(|data| data.claims)
        .map_err(|e| VerifyError::Invalid(e.to_string()))
}

fn unavailable(error: impl std::fmt::Display) -> IdpError {
    IdpError::Unavailable(error.to_string())
}

fn invalid(error: impl std::fmt::Display) -> IdpError {
    IdpError::InvalidResponse(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> OidcSettings {
        OidcSettings {
            issuer: "http://localhost:8300/realms/melior".into(),
            client_id: "melior".into(),
            scope: "openid email profile".into(),
        }
    }

    #[test]
    fn code_challenge_matches_the_rfc_7636_example() {
        assert_eq!(
            code_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn random_tokens_are_url_safe_and_unique() {
        let token = random_token(32);
        assert_eq!(token.len(), 43);
        assert!(token.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert_ne!(token, random_token(32));
    }

    #[test]
    fn authorization_url_carries_the_code_flow_parameters() {
        let url = authorization_url(
            "http://localhost:8300/realms/melior/protocol/openid-connect/auth",
            &settings(),
            "http://localhost:3000/signin",
            "/rooms/a b?relay=true",
            "challenge",
            "nonce-1",
        )
        .unwrap();
        let url = Url::parse(&url).unwrap();
        let params: Vec<(String, String)> = url.query_pairs().into_owned().collect();

        assert_eq!(url.path(), "/realms/melior/protocol/openid-connect/auth");
        for expected in [
            ("client_id", "melior"),
            ("response_type", "code"),
            ("scope", "openid email profile"),
            ("redirect_uri", "http://localhost:3000/signin"),
            ("state", "/rooms/a b?relay=true"),
            ("nonce", "nonce-1"),
            ("code_challenge", "challenge"),
            ("code_challenge_method", "S256"),
        ] {
            assert!(
                params.contains(&(expected.0.to_owned(), expected.1.to_owned())),
                "missing {expected:?} in {params:?}"
            );
        }
    }

    #[test]
    fn tokens_with_symmetric_or_no_signature_are_refused() {
        let keys = JwkSet { keys: Vec::new() };
        // {"alg":"HS256","typ":"JWT"} and {"alg":"none"} headers with dummy bodies.
        for token in [
            "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ4In0.c2ln",
            "eyJhbGciOiJub25lIn0.eyJzdWIiOiJ4In0.",
        ] {
            let result = verify_with::<serde_json::Value>(token, &keys, "issuer", None);
            assert!(matches!(result, Err(VerifyError::Invalid(_))), "{token}");
        }
    }
}
