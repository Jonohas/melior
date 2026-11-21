//! Which simulcast layer of a camera the SFU forwards to one receiver.
//!
//! A browser sends its camera to the SFU in three layers: a quarter, a half and the
//! full resolution. Every receiver gets the full layer, however big the room: a
//! bigger room is the server's load to carry, never a reason to lower quality. Only
//! a receiver's own network lowers what that receiver gets: when its reports show
//! sustained packet loss it steps down a layer, and when they stay clean it steps
//! back up. When the sender's congestion control stops sending the top layers, the
//! SFU forwards the best layer that still arrives.

/// One of a camera's simulcast layers, named by the `rid` the client gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SimulcastLayer {
    /// A quarter of the resolution, `rid` `q`.
    Low,
    /// Half the resolution, `rid` `h`.
    Medium,
    /// The full resolution, `rid` `f`.
    High,
}

/// A receiver report with more than this share of packets lost (of 256) counts as lossy.
const LOSSY_FRACTION: u8 = 26; // about 10 %
/// A receiver report with at most this share lost (of 256) counts as clean.
const CLEAN_FRACTION: u8 = 5; // about 2 %
/// Consecutive lossy reports before a receiver steps down a layer.
const LOSSY_REPORTS_TO_STEP_DOWN: u32 = 2;
/// Consecutive clean reports before a receiver steps back up a layer.
const CLEAN_REPORTS_TO_STEP_UP: u32 = 10;

impl SimulcastLayer {
    /// The layer a `rid` names; `None` for anything else.
    #[must_use]
    pub fn from_rid(rid: &str) -> Option<Self> {
        match rid {
            "q" => Some(Self::Low),
            "h" => Some(Self::Medium),
            "f" => Some(Self::High),
            _ => None,
        }
    }

    fn lower(self) -> Self {
        match self {
            Self::High => Self::Medium,
            Self::Medium | Self::Low => Self::Low,
        }
    }

    fn higher(self) -> Self {
        match self {
            Self::Low => Self::Medium,
            Self::Medium | Self::High => Self::High,
        }
    }

    /// Of the layers that arrive, the one to forward when `wanted` is asked for: the
    /// wanted one, else the best below it, else the lowest above it. `None` when no
    /// layer arrives.
    #[must_use]
    pub fn best_available(wanted: Self, available: &[Self]) -> Option<Self> {
        let below = available.iter().copied().filter(|layer| *layer <= wanted).max();
        below.or_else(|| available.iter().copied().min())
    }
}

/// The layer one receiver should get, from the packet loss its reports show. Starts
/// at the full layer; the size of the room plays no part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReceiverLayer {
    layer: SimulcastLayer,
    lossy: u32,
    clean: u32,
}

impl Default for ReceiverLayer {
    fn default() -> Self {
        Self { layer: SimulcastLayer::High, lossy: 0, clean: 0 }
    }
}

impl ReceiverLayer {
    #[must_use]
    pub fn layer(self) -> SimulcastLayer {
        self.layer
    }

    /// Takes one receiver report's `fraction_lost` (0 to 255, the RTCP scale). Sustained
    /// loss steps down one layer; a long clean run steps up one. A report in between
    /// resets both runs.
    #[must_use]
    pub fn report(self, fraction_lost: u8) -> Self {
        if fraction_lost > LOSSY_FRACTION {
            return self.after_lossy();
        }
        if fraction_lost <= CLEAN_FRACTION {
            return self.after_clean();
        }
        Self { lossy: 0, clean: 0, ..self }
    }

    fn after_lossy(self) -> Self {
        let lossy = self.lossy + 1;
        if lossy < LOSSY_REPORTS_TO_STEP_DOWN {
            return Self { lossy, clean: 0, ..self };
        }
        Self { layer: self.layer.lower(), lossy: 0, clean: 0 }
    }

    fn after_clean(self) -> Self {
        let clean = self.clean + 1;
        if clean < CLEAN_REPORTS_TO_STEP_UP {
            return Self { clean, lossy: 0, ..self };
        }
        Self { layer: self.layer.higher(), lossy: 0, clean: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::SimulcastLayer::{High, Low, Medium};
    use super::*;

    #[test]
    fn rids_name_the_three_layers() {
        assert_eq!(SimulcastLayer::from_rid("q"), Some(Low));
        assert_eq!(SimulcastLayer::from_rid("h"), Some(Medium));
        assert_eq!(SimulcastLayer::from_rid("f"), Some(High));
        assert_eq!(SimulcastLayer::from_rid(""), None);
        assert_eq!(SimulcastLayer::from_rid("x"), None);
    }

    fn reports(start: ReceiverLayer, fraction: u8, count: usize) -> ReceiverLayer {
        (0..count).fold(start, |state, _| state.report(fraction))
    }

    #[test]
    fn every_receiver_starts_at_the_full_layer() {
        assert_eq!(ReceiverLayer::default().layer(), High);
    }

    #[test]
    fn sustained_loss_steps_down_one_layer_at_a_time_and_a_blip_does_not() {
        let blip = ReceiverLayer::default().report(100);
        assert_eq!(blip.layer(), High, "one lossy report is a blip");
        let down = reports(ReceiverLayer::default(), 100, 2);
        assert_eq!(down.layer(), Medium);
        assert_eq!(reports(down, 100, 2).layer(), Low);
        assert_eq!(reports(down, 100, 20).layer(), Low, "never below the lowest");
    }

    #[test]
    fn a_long_clean_run_steps_back_up_and_a_middling_report_resets_it() {
        let low = reports(ReceiverLayer::default(), 100, 4);
        assert_eq!(low.layer(), Low);
        assert_eq!(reports(low, 0, 9).layer(), Low);
        assert_eq!(reports(low, 0, 10).layer(), Medium);
        let interrupted = reports(low, 0, 9).report(15);
        assert_eq!(reports(interrupted, 0, 9).layer(), Low, "the run starts again");
        assert_eq!(reports(low, 0, 40).layer(), High, "never above the full layer");
    }

    #[test]
    fn the_best_layer_that_arrives_stands_in_for_a_missing_one() {
        assert_eq!(SimulcastLayer::best_available(High, &[Low, Medium, High]), Some(High));
        assert_eq!(SimulcastLayer::best_available(High, &[Low, Medium]), Some(Medium));
        assert_eq!(SimulcastLayer::best_available(Medium, &[High]), Some(High));
        assert_eq!(SimulcastLayer::best_available(Low, &[Medium, High]), Some(Medium));
        assert_eq!(SimulcastLayer::best_available(Low, &[]), None);
    }
}
