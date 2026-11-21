//! RFC 2198 redundant audio ("RED"): each packet repeats the previous frames
//! before the current one, so a receiver can recover a lost packet from the next.
//!
//! A payload is a list of block headers followed by the blocks' data, oldest
//! first. A redundant block's header is four bytes (`F=1`, payload type,
//! timestamp offset, length); the last header is the primary block's one byte
//! (`F=0`, payload type) and its data runs to the end of the payload.

use std::ops::Range;

/// Set in the header of every block but the last.
const FOLLOWS: u8 = 0x80;
const PAYLOAD_TYPE: u8 = 0x7f;
const REDUNDANT_HEADER_LEN: usize = 4;

/// Where an RFC 2198 payload keeps its parts.
struct Layout {
    /// The offset of each block header; the payload type is in its low seven bits.
    headers: Vec<usize>,
    /// Where the primary block's data starts.
    primary_start: usize,
}

/// Reads the block headers. `None` when the payload is shorter than they say.
fn layout(payload: &[u8]) -> Option<Layout> {
    let mut headers = Vec::new();
    let mut position = 0;
    let mut redundant_bytes = 0;
    loop {
        let first = *payload.get(position)?;
        headers.push(position);
        if first & FOLLOWS == 0 {
            position += 1;
            break;
        }
        let high = usize::from(*payload.get(position + 2)? & 0x03);
        let low = usize::from(*payload.get(position + 3)?);
        redundant_bytes += (high << 8) | low;
        position += REDUNDANT_HEADER_LEN;
    }
    let primary_start = position + redundant_bytes;
    (primary_start <= payload.len()).then_some(Layout { headers, primary_start })
}

/// The bytes of the current frame alone: what a receiver without RED decodes.
pub(super) fn primary(payload: &[u8]) -> Option<Range<usize>> {
    Some(layout(payload)?.primary_start..payload.len())
}

/// The payload with every block marked as `payload_type`. A RED stream here only
/// ever wraps Opus, and each receiver may have numbered Opus differently from
/// the publisher.
pub(super) fn with_block_payload_type(payload: &[u8], payload_type: u8) -> Option<Vec<u8>> {
    let layout = layout(payload)?;
    let mut rewritten = payload.to_vec();
    for position in layout.headers {
        if let Some(header) = rewritten.get_mut(position) {
            *header = (*header & FOLLOWS) | (payload_type & PAYLOAD_TYPE);
        }
    }
    Some(rewritten)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A redundant block's first header byte: more blocks follow, then the payload type.
    const fn follows(payload_type: u8) -> u8 {
        0x80 | payload_type
    }

    /// One redundant block of 3 bytes (payload type 111) and a primary block of 2.
    const TWO_BLOCKS: [u8; 10] = [follows(111), 0x00, 0x3c, 0x03, 111, 1, 2, 3, 9, 8];

    #[test]
    fn the_primary_block_follows_the_redundant_data() {
        let range = primary(&TWO_BLOCKS).unwrap();
        assert_eq!(&TWO_BLOCKS[range], &[9, 8]);
    }

    #[test]
    fn a_payload_with_only_a_primary_block_is_that_block() {
        let payload = [111, 4, 5, 6];
        assert_eq!(&payload[primary(&payload).unwrap()], &[4, 5, 6]);
    }

    #[test]
    fn block_lengths_use_ten_bits() {
        let mut payload = vec![follows(111), 0x00, 0x01, 0x00, 111];
        payload.extend(std::iter::repeat_n(0, 256));
        payload.push(7);
        assert_eq!(&payload[primary(&payload).unwrap()], &[7]);
    }

    #[test]
    fn a_payload_shorter_than_its_headers_say_is_refused() {
        assert!(primary(&[]).is_none());
        assert!(primary(&[follows(111), 0x00]).is_none(), "a cut redundant header");
        assert!(primary(&[follows(111), 0x00, 0x00]).is_none(), "no length byte");
        assert!(primary(&[follows(111), 0x00, 0x00, 0x05, 111, 1]).is_none(), "data missing");
        assert!(with_block_payload_type(&[follows(111)], 109).is_none());
    }

    #[test]
    fn every_block_is_renumbered_and_the_follow_bits_stay() {
        let rewritten = with_block_payload_type(&TWO_BLOCKS, 109).unwrap();
        assert_eq!(rewritten[0], follows(109));
        assert_eq!(rewritten[4], 109);
        assert_eq!(rewritten[1..4], TWO_BLOCKS[1..4], "timestamp offset and length stay");
        assert_eq!(rewritten[5..], TWO_BLOCKS[5..], "the data stays");
    }
}
