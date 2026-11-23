//! Images for chat messages and the GIF search settings:
//!
//! - `POST /api/channels/{channel_id}/attachments`: upload an image (raw bytes).
//! - `GET /api/attachments/{id}`: an uploaded image, for readers of its channel.
//! - `GET /api/gifs/config`: the KLIPY app key browsers search with, if any.

use super::support::{Failure, caller, changing_caller, channel_id, reply};
use crate::AppState;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::HeaderMap;
use axum::http::header::{
    CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS,
};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum_extra::extract::cookie::CookieJar;
use melior_application::{CommunitiesError, ImageFile};
use melior_domain::{AttachmentId, ImageFormat, StoredImage};
use serde::Serialize;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/channels/{channel_id}/attachments",
            // Larger bodies are refused with 413 before they are read in full.
            post(upload).layer(DefaultBodyLimit::max(ImageFormat::MAX_BYTES)),
        )
        .route("/api/attachments/{id}", get(show))
        .route("/api/gifs/config", get(gif_config))
}

/// An uploaded image, to attach to a message by its id.
#[derive(Serialize)]
struct UploadedDto {
    id: String,
    content_type: &'static str,
    size: u64,
}

impl From<StoredImage> for UploadedDto {
    fn from(image: StoredImage) -> Self {
        Self {
            id: image.id.to_string(),
            content_type: image.format.content_type(),
            size: image.size,
        }
    }
}

/// `POST /api/channels/{channel_id}/attachments`: the body is the image itself.
/// Its bytes decide the format, not its `Content-Type`. Needs `view_channel` and
/// `send_messages` in the channel, text or voice.
async fn upload(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    jar: CookieJar,
    bytes: Bytes,
) -> Response {
    let (user, jar) = match changing_caller(&state, &headers, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = async {
        let id = channel_id(&id)?;
        let image = state.chat.upload_image(&id, &user, bytes.to_vec()).await?;
        Ok::<_, Failure>(UploadedDto::from(image))
    };
    reply(jar, result.await)
}

/// `GET /api/attachments/{id}`: the image, for a user who may read the history of
/// the channel it was uploaded to. Its id never changes what it names, so the
/// browser may keep it; it stays private to that browser.
async fn show(State(state): State<AppState>, Path(id): Path<String>, jar: CookieJar) -> Response {
    let (user, jar) = match caller(&state, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let result = async {
        let id = AttachmentId::parse(&id).ok_or(Failure::Community(CommunitiesError::NotFound))?;
        Ok::<_, Failure>(state.chat.image(&id, &user).await?)
    };
    match result.await {
        Ok(file) => (jar, image_response(file)).into_response(),
        Err(failure) => (jar, failure).into_response(),
    }
}

fn image_response(file: ImageFile) -> Response {
    let headers = [
        (CONTENT_TYPE, file.image.format.content_type()),
        (CACHE_CONTROL, "private, max-age=31536000, immutable"),
        // The type is the sniffed one; nothing may reinterpret it, and an image
        // opened on its own runs nothing.
        (X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (CONTENT_SECURITY_POLICY, "default-src 'none'; sandbox"),
    ];
    (headers, file.bytes).into_response()
}

/// `{ "klipy_app_key": ... }`: KLIPY wants its searches made by the browser, so
/// the browser needs the app key; `null` turns GIFs off.
#[derive(Serialize)]
struct GifConfigDto {
    klipy_app_key: Option<String>,
}

/// `GET /api/gifs/config`: for logged-in users only.
async fn gif_config(State(state): State<AppState>, jar: CookieJar) -> Response {
    let (_, jar) = match caller(&state, jar).await {
        Ok(caller) => caller,
        Err(response) => return *response,
    };
    let config = GifConfigDto { klipy_app_key: state.klipy_app_key.as_deref().map(str::to_owned) };
    reply(jar, Ok(config))
}
