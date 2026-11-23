//! The image store (`ImageStore`) on S3-compatible object storage: `RustFS` in the
//! compose stacks, any S3 API elsewhere. Requests use path-style URLs
//! (`<endpoint>/<bucket>/<key>`) signed with [`sigv4`].
//!
//! The bucket is created on the first upload that finds it missing, so the
//! server needs no setup step and starts while the store is still coming up.

pub mod sigv4;

use async_trait::async_trait;
use melior_application::{ImageStore, ImageStoreError};
use melior_domain::{AttachmentId, ImageFormat};
use reqwest::{Method, StatusCode};
use sigv4::{Credentials, Request, authorization, encode_segment, sha256_hex};
use std::time::Duration;
use time::OffsetDateTime;
use tracing::info;

/// Where images are kept within the bucket.
const KEY_PREFIX: &str = "chat-images";

/// Where the store is and how to sign for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3Settings {
    /// `http(s)://host[:port]`, without a path.
    pub endpoint: String,
    pub bucket: String,
    pub credentials: Credentials,
}

#[derive(thiserror::Error, Debug)]
pub enum S3Error {
    #[error("S3 endpoint is not an http(s) URL with a host: {0}")]
    InvalidEndpoint(String),

    #[error("S3 client: {0}")]
    Client(#[from] reqwest::Error),
}

/// [`ImageStore`] on an S3 bucket.
pub struct S3ImageStore {
    http: reqwest::Client,
    endpoint: String,
    /// The `Host` header reqwest sends for `endpoint`, which the signature covers.
    host: String,
    bucket: String,
    credentials: Credentials,
}

impl S3ImageStore {
    pub fn new(settings: S3Settings) -> Result<Self, S3Error> {
        let invalid = || S3Error::InvalidEndpoint(settings.endpoint.clone());
        let url = url::Url::parse(&settings.endpoint).map_err(|_| invalid())?;
        let host = match (url.host_str(), url.port()) {
            (Some(host), Some(port)) => format!("{host}:{port}"),
            (Some(host), None) => host.to_owned(),
            (None, _) => return Err(invalid()),
        };
        let http = reqwest::Client::builder().timeout(Duration::from_secs(30)).build()?;
        Ok(Self {
            http,
            endpoint: settings.endpoint.trim_end_matches('/').to_owned(),
            host,
            bucket: settings.bucket,
            credentials: settings.credentials,
        })
    }

    fn object_path(&self, id: &AttachmentId) -> String {
        format!("/{}/{KEY_PREFIX}/{}", encode_segment(&self.bucket), encode_segment(id.as_str()))
    }

    /// Sends a signed request with `body` to `path`.
    async fn send(
        &self,
        method: Method,
        path: &str,
        body: Vec<u8>,
        content_type: Option<&str>,
    ) -> Result<reqwest::Response, ImageStoreError> {
        let payload_hash = sha256_hex(&body);
        let amz_date = amz_date(OffsetDateTime::now_utc());
        let headers = [
            ("host", self.host.as_str()),
            ("x-amz-content-sha256", payload_hash.as_str()),
            ("x-amz-date", amz_date.as_str()),
        ];
        let signed = Request {
            method: method.as_str(),
            path,
            headers: &headers,
            payload_hash: &payload_hash,
            amz_date: &amz_date,
        };
        let mut request = self
            .http
            .request(method.clone(), format!("{}{path}", self.endpoint))
            .header("authorization", authorization(&signed, &self.credentials))
            .header("x-amz-content-sha256", &payload_hash)
            .header("x-amz-date", &amz_date)
            .body(body);
        if let Some(content_type) = content_type {
            request = request.header("content-type", content_type);
        }
        request.send().await.map_err(unavailable)
    }

    async fn put_object(
        &self,
        id: &AttachmentId,
        format: ImageFormat,
        bytes: Vec<u8>,
    ) -> Result<StatusCode, ImageStoreError> {
        let path = self.object_path(id);
        let response = self.send(Method::PUT, &path, bytes, Some(format.content_type())).await?;
        Ok(response.status())
    }

    async fn create_bucket(&self) -> Result<(), ImageStoreError> {
        let path = format!("/{}", encode_segment(&self.bucket));
        let status = self.send(Method::PUT, &path, Vec::new(), None).await?.status();
        // 409: another instance created it first.
        if status.is_success() || status == StatusCode::CONFLICT {
            info!(bucket = %self.bucket, "image bucket created");
            return Ok(());
        }
        Err(refused("create bucket", status))
    }
}

#[async_trait]
impl ImageStore for S3ImageStore {
    async fn put(
        &self,
        id: &AttachmentId,
        format: ImageFormat,
        bytes: Vec<u8>,
    ) -> Result<(), ImageStoreError> {
        let mut status = self.put_object(id, format, bytes.clone()).await?;
        if status == StatusCode::NOT_FOUND {
            self.create_bucket().await?;
            status = self.put_object(id, format, bytes).await?;
        }
        if !status.is_success() {
            return Err(refused("put object", status));
        }
        Ok(())
    }

    async fn get(&self, id: &AttachmentId) -> Result<Option<Vec<u8>>, ImageStoreError> {
        let response = self.send(Method::GET, &self.object_path(id), Vec::new(), None).await?;
        let status = response.status();
        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(refused("get object", status));
        }
        let bytes = response.bytes().await.map_err(unavailable)?;
        Ok(Some(bytes.to_vec()))
    }

    async fn delete(&self, id: &AttachmentId) -> Result<(), ImageStoreError> {
        let response = self.send(Method::DELETE, &self.object_path(id), Vec::new(), None).await?;
        let status = response.status();
        // S3 answers 204 whether or not the object existed; 404 for no bucket.
        if status.is_success() || status == StatusCode::NOT_FOUND {
            return Ok(());
        }
        Err(refused("delete object", status))
    }
}

/// `YYYYMMDDTHHMMSSZ`.
fn amz_date(now: OffsetDateTime) -> String {
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

/// A transport error, without the request URL.
fn unavailable(error: reqwest::Error) -> ImageStoreError {
    ImageStoreError::Unavailable(error.without_url().to_string())
}

fn refused(action: &str, status: StatusCode) -> ImageStoreError {
    ImageStoreError::Unavailable(format!("{action} refused with HTTP {status}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(endpoint: &str) -> S3Settings {
        S3Settings {
            endpoint: endpoint.into(),
            bucket: "melior-images".into(),
            credentials: Credentials {
                access_key: "ak".into(),
                secret_key: "sk".into(),
                region: "us-east-1".into(),
            },
        }
    }

    #[test]
    fn the_signed_host_carries_a_port_only_when_it_is_not_the_default() {
        let local = S3ImageStore::new(settings("http://127.0.0.1:9000/")).unwrap();
        assert_eq!(local.host, "127.0.0.1:9000");
        assert_eq!(local.endpoint, "http://127.0.0.1:9000");
        let default = S3ImageStore::new(settings("https://s3.example.org")).unwrap();
        assert_eq!(default.host, "s3.example.org");
        assert!(S3ImageStore::new(settings("not a url")).is_err());
    }

    #[test]
    fn objects_live_under_the_prefix_in_the_bucket() {
        let store = S3ImageStore::new(settings("http://127.0.0.1:9000")).unwrap();
        let id = AttachmentId::generate();
        assert_eq!(store.object_path(&id), format!("/melior-images/chat-images/{id}"));
    }

    #[test]
    fn dates_are_in_the_basic_iso_format() {
        let at = OffsetDateTime::from_unix_timestamp(1_369_353_600).unwrap();
        assert_eq!(amz_date(at), "20130524T000000Z");
    }
}
