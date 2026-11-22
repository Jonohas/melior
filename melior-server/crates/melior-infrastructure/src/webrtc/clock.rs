//! Sender-report clocks: which RTP time a stream had at which wall-clock time.
//!
//! A browser lines audio up with video by the (NTP, RTP) pairs of each stream's
//! RTCP sender reports. The SFU must therefore pass on the publisher's pairs, not
//! make its own from when packets happen to pass through: that would hide how
//! much longer the publisher took to encode a frame than the sound that goes with
//! it, which is exactly what grows when its CPU is busy.
//!
//! Every time here is an NTP timestamp (32.32 fixed-point seconds) on the SFU's
//! own clock. A publisher's times are moved onto it by one offset per publisher,
//! so its streams keep their relation to each other, and an outlet that moves
//! from one publisher to the next keeps one timeline.

use std::sync::{Mutex, PoisonError};

/// One second in NTP units.
const NTP_SECOND: i128 = 1 << 32;

/// The low 32 bits of `value`, as RTP timestamps wrap.
fn wrap_u32(value: i128) -> u32 {
    u32::try_from(value.rem_euclid(1 << 32)).unwrap_or(0)
}

/// The low 64 bits of `value`, as NTP timestamps wrap.
fn wrap_u64(value: i128) -> u64 {
    u64::try_from(value.rem_euclid(1 << 64)).unwrap_or(0)
}

/// The relation between one stream's RTP timestamps and the SFU's wall clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct StreamClock {
    ntp: u64,
    rtp: u32,
    /// RTP ticks per second.
    rate: u32,
}

impl StreamClock {
    pub fn new(ntp: u64, rtp: u32, rate: u32) -> Self {
        Self { ntp, rtp, rate }
    }

    /// The stream's RTP timestamp at wall-clock time `ntp`.
    pub fn rtp_at(self, ntp: u64) -> u32 {
        let elapsed = i128::from(ntp) - i128::from(self.ntp);
        let ticks = (elapsed * i128::from(self.rate)).div_euclid(NTP_SECOND);
        wrap_u32(i128::from(self.rtp) + ticks)
    }

    /// The same clock after every timestamp was moved by `offset` ticks, as an
    /// outlet rewrites them.
    pub fn shifted(self, offset: u32) -> Self {
        Self { rtp: self.rtp.wrapping_add(offset), ..self }
    }

    /// The timestamp offset that puts `next` on this clock's timeline: a packet of
    /// `next`, moved by it, carries the time this clock had when it was captured.
    pub fn offset_for(self, next: Self) -> u32 {
        self.rtp_at(next.ntp).wrapping_sub(next.rtp)
    }
}

/// Moves one publisher's NTP times onto the SFU's clock. The offset is fixed at the
/// publisher's first sender report and serves all its streams, so they stay in
/// step with each other exactly as the publisher reported them.
#[derive(Debug, Default)]
pub(super) struct PublisherClock {
    offset: Mutex<Option<i128>>,
}

impl PublisherClock {
    /// `remote`, a time on the publisher's clock, on the SFU's. `local_now` is the
    /// SFU's time as the report carrying `remote` arrived.
    pub fn to_local(&self, remote: u64, local_now: u64) -> u64 {
        let mut offset = self.offset.lock().unwrap_or_else(PoisonError::into_inner);
        let offset = *offset.get_or_insert(i128::from(local_now) - i128::from(remote));
        wrap_u64(i128::from(remote) + offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: u64 = 1 << 32;

    #[test]
    fn a_clock_runs_at_its_rate_both_ways() {
        let clock = StreamClock::new(100 * SECOND, 1_000, 90_000);
        assert_eq!(clock.rtp_at(100 * SECOND), 1_000);
        assert_eq!(clock.rtp_at(101 * SECOND), 91_000);
        assert_eq!(clock.rtp_at(100 * SECOND + SECOND / 2), 46_000);
        assert_eq!(clock.rtp_at(99 * SECOND), 1_000u32.wrapping_sub(90_000));
    }

    #[test]
    fn timestamps_wrap() {
        let clock = StreamClock::new(10 * SECOND, u32::MAX - 10, 48_000);
        assert_eq!(clock.rtp_at(11 * SECOND), 47_989);
    }

    #[test]
    fn a_shifted_clock_carries_the_offset() {
        let clock = StreamClock::new(5 * SECOND, 100, 90_000).shifted(50);
        assert_eq!(clock.rtp_at(6 * SECOND), 90_150);
    }

    #[test]
    fn an_aligned_offset_continues_the_old_timeline() {
        // The outlet's old timeline, and a new layer with a timestamp base of its own.
        let old = StreamClock::new(100 * SECOND, 7_000, 90_000);
        let next = StreamClock::new(102 * SECOND, 3_000_000_000, 90_000);
        let offset = old.offset_for(next);

        let at = 103 * SECOND;
        assert_eq!(next.shifted(offset).rtp_at(at), old.rtp_at(at));
    }

    #[test]
    fn one_offset_serves_every_stream_of_a_publisher() {
        let publisher = PublisherClock::default();
        // The first report fixes the offset: the publisher's clock is 50 s behind.
        assert_eq!(publisher.to_local(10 * SECOND, 60 * SECOND), 60 * SECOND);
        // A later report of another stream keeps it, whenever it arrives.
        assert_eq!(publisher.to_local(12 * SECOND, 63 * SECOND), 62 * SECOND);
    }
}
