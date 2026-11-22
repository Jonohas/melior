//! The mechanics of forwarding one simulcast layer at a time: finding where a VP8
//! keyframe starts, and deciding per packet whether an outlet sends it. Which layer
//! an outlet wants is the domain's rule ([`melior_domain::ReceiverLayer`]).

use melior_domain::SimulcastLayer;

/// Whether an RTP payload is the first packet of a VP8 keyframe.
///
/// It reads the VP8 payload descriptor (RFC 7741, 4.2) and then the first byte of
/// the VP8 frame tag, whose lowest bit is 0 for a keyframe (RFC 6386, 9.1). End-to-end
/// encryption keeps that byte in the clear, so this works on encrypted frames too.
#[must_use]
pub fn vp8_keyframe_start(payload: &[u8]) -> bool {
    let Some(&first) = payload.first() else {
        return false;
    };
    let starts_partition = first & 0x10 != 0;
    // The partition index (PID) is the low three bits.
    let partition_zero = first.trailing_zeros() >= 3;
    if !starts_partition || !partition_zero {
        return false;
    }
    let Some(header) = descriptor_length(payload) else {
        return false;
    };
    payload.get(header).is_some_and(|tag| tag & 0x01 == 0)
}

/// The length of the VP8 payload descriptor, or `None` when the payload ends inside it.
fn descriptor_length(payload: &[u8]) -> Option<usize> {
    let first = *payload.first()?;
    if first & 0x80 == 0 {
        return Some(1);
    }
    let extension = *payload.get(1)?;
    let mut length = 2;
    if extension & 0x80 != 0 {
        let picture_id = *payload.get(length)?;
        length += if picture_id & 0x80 != 0 { 2 } else { 1 };
    }
    if extension & 0x40 != 0 {
        length += 1;
    }
    if extension & 0x30 != 0 {
        length += 1;
    }
    Some(length)
}

/// What an outlet does with one packet of one layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Send it: it belongs to the layer the outlet carries.
    Write,
    /// Switch to this packet's layer, starting with this keyframe, and send it.
    SwitchAndWrite,
    /// Drop it: another layer is being carried, or this one has no keyframe yet.
    Skip,
}

/// Which layer an outlet carries now, and which it should carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerChoice {
    pub current: Option<SimulcastLayer>,
    pub target: Option<SimulcastLayer>,
}

impl LayerChoice {
    /// Decides on a packet of `layer`. An outlet keeps sending its current layer until
    /// the target layer's next keyframe arrives, so the picture never breaks.
    #[must_use]
    pub fn step(self, layer: SimulcastLayer, keyframe_start: bool) -> Step {
        let switching = self.target != self.current;
        if self.current == Some(layer) {
            return Step::Write;
        }
        if switching && self.target == Some(layer) && keyframe_start {
            return Step::SwitchAndWrite;
        }
        Step::Skip
    }

    /// Whether the outlet waits for a keyframe of its target layer.
    #[must_use]
    pub fn waiting(self) -> bool {
        self.target.is_some() && self.target != self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use SimulcastLayer::{High, Low, Medium};

    #[test]
    fn finds_the_start_of_a_keyframe_behind_every_descriptor_shape() {
        // S=1, PID=0, no extension; frame tag 0x10: keyframe.
        assert!(vp8_keyframe_start(&[0x10, 0x10, 0x02, 0x00]));
        // X=1 with a 15-bit picture id, TL0PICIDX and TID/KEYIDX.
        assert!(vp8_keyframe_start(&[0x90, 0xf0, 0x80, 0x01, 0x05, 0x20, 0x50, 0x02]));
        // X=1 with a 7-bit picture id only.
        assert!(vp8_keyframe_start(&[0x90, 0x80, 0x12, 0x30]));
    }

    #[test]
    fn rejects_delta_frames_continuations_and_short_payloads() {
        assert!(!vp8_keyframe_start(&[0x10, 0x11]), "delta frame");
        assert!(!vp8_keyframe_start(&[0x00, 0x10]), "not the start of a partition");
        assert!(!vp8_keyframe_start(&[0x11, 0x10]), "partition 1");
        assert!(!vp8_keyframe_start(&[0x90, 0x80]), "ends inside the descriptor");
        assert!(!vp8_keyframe_start(&[]));
    }

    #[test]
    fn an_outlet_switches_only_at_a_keyframe_of_its_target_and_keeps_its_layer_until_then() {
        let moving = LayerChoice { current: Some(Low), target: Some(High) };
        assert_eq!(moving.step(Low, false), Step::Write, "keeps the old layer meanwhile");
        assert_eq!(moving.step(High, false), Step::Skip, "no keyframe yet");
        assert_eq!(moving.step(High, true), Step::SwitchAndWrite);
        assert_eq!(moving.step(Medium, true), Step::Skip, "not the target");
        assert!(moving.waiting());
    }

    #[test]
    fn a_settled_outlet_sends_its_layer_and_nothing_else() {
        let settled = LayerChoice { current: Some(Medium), target: Some(Medium) };
        assert_eq!(settled.step(Medium, false), Step::Write);
        assert_eq!(settled.step(High, true), Step::Skip);
        assert!(!settled.waiting());
    }

    #[test]
    fn a_new_outlet_starts_at_the_first_keyframe_of_its_target() {
        let fresh = LayerChoice { current: None, target: Some(High) };
        assert_eq!(fresh.step(High, false), Step::Skip);
        assert_eq!(fresh.step(High, true), Step::SwitchAndWrite);
    }
}
