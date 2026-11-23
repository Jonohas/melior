use super::{Attachment, ChatError};
use crate::ChannelId;
use std::fmt;

/// Orders the messages of a room. Assigned by the store when a message is
/// saved, so it grows with every message and never repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MessageId(i64);

impl MessageId {
    #[must_use]
    pub fn new(value: i64) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn value(self) -> i64 {
        self.0
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// The text of a chat message, checked. Comes from a client, so nothing else
/// builds one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatText(String);

impl ChatText {
    pub const MAX_CHARS: usize = 2000;

    /// Trims the text and checks it: not empty, at most [`Self::MAX_CHARS`]
    /// characters, and no control characters other than line breaks and tabs.
    pub fn parse(raw: &str) -> Result<Self, ChatError> {
        let text = raw.trim();
        if text.is_empty() {
            return Err(ChatError::Empty);
        }
        if text.chars().count() > Self::MAX_CHARS {
            return Err(ChatError::TooLong);
        }
        if text.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\t')) {
            return Err(ChatError::InvalidCharacter);
        }
        Ok(Self(text.to_owned()))
    }

    /// Like [`Self::parse`], except that blank text is no text rather than an
    /// error: a message with an attachment needs none.
    pub fn parse_optional(raw: &str) -> Result<Option<Self>, ChatError> {
        match Self::parse(raw) {
            Ok(text) => Ok(Some(text)),
            Err(ChatError::Empty) => Ok(None),
            Err(error) => Err(error),
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ChatText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a message says: text, an attachment, or both, never neither.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatContent {
    text: Option<ChatText>,
    attachment: Option<Attachment>,
}

impl ChatContent {
    /// Content of checked text and an attachment; refused as empty when both are missing.
    pub fn new(text: Option<ChatText>, attachment: Option<Attachment>) -> Result<Self, ChatError> {
        if text.is_none() && attachment.is_none() {
            return Err(ChatError::Empty);
        }
        Ok(Self { text, attachment })
    }

    #[must_use]
    pub fn text(&self) -> Option<&ChatText> {
        self.text.as_ref()
    }

    /// The text, or `""` for a message that has only an attachment.
    #[must_use]
    pub fn text_str(&self) -> &str {
        self.text.as_ref().map_or("", ChatText::as_str)
    }

    #[must_use]
    pub fn attachment(&self) -> Option<&Attachment> {
        self.attachment.as_ref()
    }
}

impl From<ChatText> for ChatContent {
    fn from(text: ChatText) -> Self {
        Self { text: Some(text), attachment: None }
    }
}

/// A message that was said in a room and saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub id: MessageId,
    pub channel_id: ChannelId,
    /// Username of the author.
    pub author: String,
    pub content: ChatContent,
    /// When the store saved it, in milliseconds since the Unix epoch.
    pub sent_at_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_trimmed() {
        assert_eq!(ChatText::parse("  hi there \n").map(|t| t.to_string()), Ok("hi there".into()));
    }

    #[test]
    fn empty_and_blank_text_is_refused() {
        assert_eq!(ChatText::parse(""), Err(ChatError::Empty));
        assert_eq!(ChatText::parse(" \n\t "), Err(ChatError::Empty));
    }

    #[test]
    fn the_limit_counts_characters_not_bytes() {
        let at_limit = "é".repeat(ChatText::MAX_CHARS);
        assert!(ChatText::parse(&at_limit).is_ok());
        assert_eq!(ChatText::parse(&format!("{at_limit}x")), Err(ChatError::TooLong));
    }

    #[test]
    fn line_breaks_and_tabs_stay_but_other_control_characters_do_not() {
        assert!(ChatText::parse("one\ntwo\tthree").is_ok());
        assert_eq!(ChatText::parse("bell\u{7}"), Err(ChatError::InvalidCharacter));
        assert_eq!(ChatText::parse("nul\0"), Err(ChatError::InvalidCharacter));
    }

    #[test]
    fn blank_text_is_no_text_when_it_may_be_missing() {
        assert_eq!(ChatText::parse_optional(" \n "), Ok(None));
        assert_eq!(
            ChatText::parse_optional(" hi ").map(|t| t.map(|t| t.to_string())),
            Ok(Some("hi".into()))
        );
        assert_eq!(ChatText::parse_optional("bell\u{7}"), Err(ChatError::InvalidCharacter));
    }

    #[test]
    fn content_needs_text_or_an_attachment() {
        assert_eq!(ChatContent::new(None, None), Err(ChatError::Empty));
        let gif = crate::GifLink::parse("https://static.klipy.com/a.gif", 1, 1).unwrap();
        let only_gif = ChatContent::new(None, Some(Attachment::Gif(gif))).unwrap();
        assert_eq!(only_gif.text_str(), "");
        assert!(only_gif.attachment().is_some());
        let only_text = ChatContent::from(ChatText::parse("hi").unwrap());
        assert_eq!((only_text.text_str(), only_text.attachment()), ("hi", None));
    }

    #[test]
    fn message_ids_order_by_value() {
        assert!(MessageId::new(2) > MessageId::new(1));
    }
}
