//! What a chat message can carry besides its text: an uploaded image or a GIF
//! from the GIF provider (KLIPY), which browsers load from the provider directly.

use super::ChatError;
use crate::ChannelId;
use crate::community::uuid_id;
use std::fmt;

uuid_id!(
    /// Identifies an uploaded image. The server generates it, so it also names
    /// the stored object.
    AttachmentId
);

/// The image formats chat accepts, recognised by the file's first bytes rather
/// than by what the client claims. SVG is not one of them: it can carry script.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Gif,
    Webp,
}

impl ImageFormat {
    /// The largest image that may be uploaded: 10 MiB.
    pub const MAX_BYTES: usize = 10 * 1024 * 1024;

    /// The format whose signature `bytes` start with, if any.
    #[must_use]
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Some(Self::Png);
        }
        if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Some(Self::Jpeg);
        }
        if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            return Some(Self::Gif);
        }
        let is_webp = bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP";
        is_webp.then_some(Self::Webp)
    }

    /// Checks an upload: at most [`Self::MAX_BYTES`], and one of the formats by
    /// its content.
    pub fn check_upload(bytes: &[u8]) -> Result<Self, ChatError> {
        if bytes.len() > Self::MAX_BYTES {
            return Err(ChatError::ImageTooLarge);
        }
        Self::sniff(bytes).ok_or(ChatError::UnsupportedImage)
    }

    /// The media type an image of this format is served with.
    #[must_use]
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
        }
    }

    /// The format with this media type, as [`Self::content_type`] spells it.
    #[must_use]
    pub fn from_content_type(raw: &str) -> Option<Self> {
        [Self::Png, Self::Jpeg, Self::Gif, Self::Webp]
            .into_iter()
            .find(|format| format.content_type() == raw)
    }
}

impl fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.content_type())
    }
}

/// An uploaded image as it is recorded: which channel it was uploaded to and by
/// whom. Its bytes are stored separately, under its id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredImage {
    pub id: AttachmentId,
    pub channel_id: ChannelId,
    /// Username of the member who uploaded it.
    pub uploader: String,
    pub format: ImageFormat,
    pub size: u64,
}

impl StoredImage {
    /// The attachment a message by `author` in `channel_id` gets for this image.
    /// Only the uploader may attach it, and only in the channel it was uploaded
    /// to: who may see an image is decided by that channel. Any other case reads
    /// as an unknown image, so an id says nothing about images of others.
    pub fn attach(&self, channel_id: &ChannelId, author: &str) -> Result<Attachment, ChatError> {
        if self.channel_id != *channel_id || self.uploader != author {
            return Err(ChatError::UnknownImage);
        }
        Ok(Attachment::Image { id: self.id.clone(), format: self.format })
    }
}

/// A GIF on the GIF provider's media servers, with its size for the layout.
/// Clients load it from there directly, so only those hosts are accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GifLink {
    url: String,
    width: u16,
    height: u16,
}

impl GifLink {
    /// KLIPY's media hosts.
    pub const HOSTS: [&'static str; 3] =
        ["static.klipy.com", "static1.klipy.com", "static2.klipy.com"];
    pub const MAX_URL_CHARS: usize = 1024;
    /// The largest width or height a GIF may claim.
    pub const MAX_SIDE: u16 = 4096;

    /// Checks a GIF a client wants to send: an `https` URL on one of
    /// [`Self::HOSTS`] with a path, at most [`Self::MAX_URL_CHARS`], without
    /// whitespace, control or quoting characters; each side 1 to [`Self::MAX_SIDE`].
    pub fn parse(url: &str, width: u32, height: u32) -> Result<Self, ChatError> {
        let width = side(width)?;
        let height = side(height)?;
        if url.chars().count() > Self::MAX_URL_CHARS || url.chars().any(unsafe_in_url) {
            return Err(ChatError::InvalidGif);
        }
        let rest = url.strip_prefix("https://").ok_or(ChatError::InvalidGif)?;
        let (host, path) = rest.split_once('/').ok_or(ChatError::InvalidGif)?;
        if !Self::HOSTS.contains(&host) || path.is_empty() {
            return Err(ChatError::InvalidGif);
        }
        Ok(Self { url: url.to_owned(), width, height })
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    #[must_use]
    pub fn width(&self) -> u16 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u16 {
        self.height
    }
}

fn side(value: u32) -> Result<u16, ChatError> {
    match u16::try_from(value) {
        Ok(side) if (1..=GifLink::MAX_SIDE).contains(&side) => Ok(side),
        _ => Err(ChatError::InvalidGif),
    }
}

fn unsafe_in_url(c: char) -> bool {
    c.is_whitespace() || c.is_control() || matches!(c, '"' | '\'' | '<' | '>' | '\\' | '`')
}

/// What a saved message carries besides its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attachment {
    /// An uploaded image, served by the server to those who may read the channel.
    Image {
        id: AttachmentId,
        format: ImageFormat,
    },
    Gif(GifLink),
}

/// What a client asks to attach to a message, before the image it names has
/// been looked up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachmentRequest {
    Image(AttachmentId),
    Gif(GifLink),
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIF_URL: &str = "https://static.klipy.com/ii/935d7ab9/14/af/um0L4dFH.gif";

    fn webp_header() -> Vec<u8> {
        let mut bytes = b"RIFF\0\0\0\0WEBPVP8 ".to_vec();
        bytes.extend_from_slice(&[0; 8]);
        bytes
    }

    #[test]
    fn formats_are_recognised_by_their_first_bytes() {
        assert_eq!(ImageFormat::sniff(b"\x89PNG\r\n\x1a\nrest"), Some(ImageFormat::Png));
        assert_eq!(ImageFormat::sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some(ImageFormat::Jpeg));
        assert_eq!(ImageFormat::sniff(b"GIF89a..."), Some(ImageFormat::Gif));
        assert_eq!(ImageFormat::sniff(b"GIF87a..."), Some(ImageFormat::Gif));
        assert_eq!(ImageFormat::sniff(&webp_header()), Some(ImageFormat::Webp));
    }

    #[test]
    fn svg_html_truncated_and_empty_files_are_not_images() {
        assert_eq!(ImageFormat::sniff(b"<svg xmlns='http://www.w3.org/2000/svg'/>"), None);
        assert_eq!(ImageFormat::sniff(b"<html><script>"), None);
        assert_eq!(ImageFormat::sniff(b"RIFF\0\0\0\0WEB"), None);
        assert_eq!(ImageFormat::sniff(b""), None);
        assert_eq!(ImageFormat::check_upload(b""), Err(ChatError::UnsupportedImage));
    }

    #[test]
    fn an_upload_is_at_most_ten_mebibytes() {
        let mut at_limit = b"GIF89a".to_vec();
        at_limit.resize(ImageFormat::MAX_BYTES, 0);
        assert_eq!(ImageFormat::check_upload(&at_limit), Ok(ImageFormat::Gif));
        at_limit.push(0);
        assert_eq!(ImageFormat::check_upload(&at_limit), Err(ChatError::ImageTooLarge));
    }

    #[test]
    fn content_types_round_trip() {
        for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::Gif, ImageFormat::Webp] {
            assert_eq!(ImageFormat::from_content_type(format.content_type()), Some(format));
        }
        assert_eq!(ImageFormat::from_content_type("image/svg+xml"), None);
    }

    fn stored() -> StoredImage {
        StoredImage {
            id: AttachmentId::generate(),
            channel_id: ChannelId::parse("general").unwrap(),
            uploader: "alice".into(),
            format: ImageFormat::Png,
            size: 10,
        }
    }

    #[test]
    fn only_the_uploader_attaches_an_image_and_only_in_its_channel() {
        let image = stored();
        let general = ChannelId::parse("general").unwrap();
        assert_eq!(
            image.attach(&general, "alice"),
            Ok(Attachment::Image { id: image.id.clone(), format: ImageFormat::Png })
        );
        assert_eq!(image.attach(&general, "bob"), Err(ChatError::UnknownImage));
        let other = ChannelId::parse("other").unwrap();
        assert_eq!(image.attach(&other, "alice"), Err(ChatError::UnknownImage));
    }

    #[test]
    fn a_gif_comes_from_klipys_media_hosts() {
        let gif = GifLink::parse(GIF_URL, 498, 280).unwrap();
        assert_eq!((gif.url(), gif.width(), gif.height()), (GIF_URL, 498, 280));
        for host in GifLink::HOSTS {
            assert!(GifLink::parse(&format!("https://{host}/a.gif"), 1, 1).is_ok());
        }
    }

    #[test]
    fn other_hosts_schemes_and_disguised_hosts_are_refused() {
        for url in [
            "http://static.klipy.com/a.gif",
            "https://evil.example/a.gif",
            "https://static.klipy.com.evil.example/a.gif",
            "https://static.klipy.com@evil.example/a.gif",
            "https://static.klipy.com:8443/a.gif",
            "https://static.klipy.com",
            "https://static.klipy.com/",
            "javascript:alert(1)",
            "https://static.klipy.com/a b.gif",
            "https://static.klipy.com/a\".gif",
            "https://static.klipy.com/a<script>.gif",
        ] {
            assert_eq!(GifLink::parse(url, 10, 10), Err(ChatError::InvalidGif), "{url}");
        }
        let long = format!("https://static.klipy.com/{}", "a".repeat(GifLink::MAX_URL_CHARS));
        assert_eq!(GifLink::parse(&long, 10, 10), Err(ChatError::InvalidGif));
    }

    #[test]
    fn a_gif_side_is_one_to_4096() {
        assert!(GifLink::parse(GIF_URL, 4096, 1).is_ok());
        assert_eq!(GifLink::parse(GIF_URL, 0, 10), Err(ChatError::InvalidGif));
        assert_eq!(GifLink::parse(GIF_URL, 10, 4097), Err(ChatError::InvalidGif));
        assert_eq!(GifLink::parse(GIF_URL, u32::MAX, 10), Err(ChatError::InvalidGif));
    }
}
