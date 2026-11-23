//! AWS Signature Version 4 for S3 requests with path-style URLs: what `RustFS` and
//! every other S3-compatible store accepts. Only what this adapter sends is
//! covered: no query string, a fixed list of signed headers.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

/// The key pair and region requests are signed for.
#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
}

/// The secret key is left out.
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("access_key", &self.access_key)
            .field("region", &self.region)
            .finish_non_exhaustive()
    }
}

/// A request to sign. `headers` are the signed headers, names in lowercase;
/// they must include `host`, `x-amz-content-sha256` and `x-amz-date`.
pub struct Request<'a> {
    pub method: &'a str,
    /// The URI-encoded path, starting with `/`.
    pub path: &'a str,
    pub headers: &'a [(&'a str, &'a str)],
    /// Hex SHA-256 of the body, as in `x-amz-content-sha256`.
    pub payload_hash: &'a str,
    /// `YYYYMMDDTHHMMSSZ`, as in `x-amz-date`.
    pub amz_date: &'a str,
}

/// The `Authorization` header value for `request`.
#[must_use]
pub fn authorization(request: &Request<'_>, credentials: &Credentials) -> String {
    let date = request.amz_date.get(..8).unwrap_or_default();
    let scope = format!("{date}/{}/s3/aws4_request", credentials.region);
    let mut headers = request.headers.to_vec();
    headers.sort_unstable_by_key(|(name, _)| *name);
    let signed_headers = headers.iter().map(|(name, _)| *name).collect::<Vec<_>>().join(";");
    let canonical_headers = headers.iter().fold(String::new(), |mut out, (name, value)| {
        let _ = writeln!(out, "{name}:{}", value.trim());
        out
    });
    let canonical_request = format!(
        "{}\n{}\n\n{canonical_headers}\n{signed_headers}\n{}",
        request.method, request.path, request.payload_hash
    );
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{scope}\n{}",
        request.amz_date,
        sha256_hex(canonical_request.as_bytes())
    );
    let mut key = hmac(format!("AWS4{}", credentials.secret_key).as_bytes(), date.as_bytes());
    for part in [credentials.region.as_str(), "s3", "aws4_request"] {
        key = hmac(&key, part.as_bytes());
    }
    let signature = hex(&hmac(&key, string_to_sign.as_bytes()));
    format!(
        "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
        credentials.access_key
    )
}

#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// Encodes a path segment as `SigV4` wants it: everything but unreserved
/// characters percent-encoded.
#[must_use]
pub fn encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
            continue;
        }
        let _ = write!(out, "%{byte:02X}");
    }
    out
}

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    // HMAC accepts a key of any length, so this cannot fail; an empty MAC (and
    // a request the store refuses) is the fallback rather than a panic.
    Hmac::<Sha256>::new_from_slice(key)
        .map(|mut mac| {
            mac.update(data);
            mac.finalize().into_bytes().to_vec()
        })
        .unwrap_or_default()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    /// The "GET Object" example of the AWS documentation, "Signature Calculations
    /// for the Authorization Header: Transferring Payload in a Single Chunk".
    #[test]
    fn signs_the_documented_get_object_example() {
        let credentials = Credentials {
            access_key: "AKIAIOSFODNN7EXAMPLE".into(),
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".into(),
            region: "us-east-1".into(),
        };
        let request = Request {
            method: "GET",
            path: "/test.txt",
            headers: &[
                ("x-amz-date", "20130524T000000Z"),
                ("host", "examplebucket.s3.amazonaws.com"),
                ("range", "bytes=0-9"),
                ("x-amz-content-sha256", EMPTY_SHA256),
            ],
            payload_hash: EMPTY_SHA256,
            amz_date: "20130524T000000Z",
        };

        assert_eq!(
            authorization(&request, &credentials),
            "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, \
             SignedHeaders=host;range;x-amz-content-sha256;x-amz-date, \
             Signature=f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41"
        );
    }

    #[test]
    fn hashes_and_encodes() {
        assert_eq!(sha256_hex(b""), EMPTY_SHA256);
        assert_eq!(encode_segment("a b/c~d.e_f-g"), "a%20b%2Fc~d.e_f-g");
    }

    #[test]
    fn debug_hides_the_secret_key() {
        let credentials =
            Credentials { access_key: "ak".into(), secret_key: "hush".into(), region: "r".into() };
        assert!(!format!("{credentials:?}").contains("hush"));
    }
}
