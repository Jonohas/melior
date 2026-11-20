//! A token bucket over an injected clock, used to rate-limit what one connection
//! may ask the server to relay.

use std::time::Instant;

/// Messages a connection may relay in one burst.
pub const RELAY_BURST: u32 = 128;
/// Messages a connection may relay per second once its burst is spent.
pub const RELAY_PER_SECOND: u32 = 64;

#[derive(Debug, Clone)]
pub struct TokenBucket {
    capacity: f64,
    per_second: f64,
    tokens: f64,
    refilled_at: Instant,
}

impl TokenBucket {
    /// A full bucket as of `now`.
    #[must_use]
    pub fn new(capacity: u32, per_second: u32, now: Instant) -> Self {
        Self {
            capacity: f64::from(capacity),
            per_second: f64::from(per_second),
            tokens: f64::from(capacity),
            refilled_at: now,
        }
    }

    /// The bucket for relay messages, full as of `now`.
    #[must_use]
    pub fn for_relay(now: Instant) -> Self {
        Self::new(RELAY_BURST, RELAY_PER_SECOND, now)
    }

    /// Takes one token if there is one. A clock that went backwards adds nothing.
    pub fn try_take(&mut self, now: Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.refilled_at);
        self.refilled_at = self.refilled_at.max(now);
        self.tokens = (self.tokens + elapsed.as_secs_f64() * self.per_second).min(self.capacity);
        if self.tokens < 1.0 {
            return false;
        }
        self.tokens -= 1.0;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn drain(bucket: &mut TokenBucket, now: Instant) -> u32 {
        let mut taken = 0;
        while bucket.try_take(now) {
            taken += 1;
        }
        taken
    }

    #[test]
    fn a_full_bucket_allows_its_burst_and_then_refuses() {
        let start = Instant::now();
        let mut bucket = TokenBucket::new(5, 1, start);
        assert_eq!(drain(&mut bucket, start), 5);
        assert!(!bucket.try_take(start));
    }

    #[test]
    fn tokens_return_with_time_up_to_the_capacity() {
        let start = Instant::now();
        let mut bucket = TokenBucket::new(5, 2, start);
        drain(&mut bucket, start);
        assert_eq!(drain(&mut bucket, start + Duration::from_secs(1)), 2);
        assert_eq!(drain(&mut bucket, start + Duration::from_secs(1000)), 5);
    }

    #[test]
    fn a_clock_that_goes_backwards_adds_nothing() {
        let start = Instant::now() + Duration::from_secs(10);
        let mut bucket = TokenBucket::new(1, 1, start);
        assert!(bucket.try_take(start));
        assert!(!bucket.try_take(start.checked_sub(Duration::from_secs(5)).unwrap()));
    }

    #[test]
    fn the_relay_bucket_holds_the_burst_of_an_ice_trickle() {
        let start = Instant::now();
        let mut bucket = TokenBucket::for_relay(start);
        assert_eq!(drain(&mut bucket, start), RELAY_BURST);
    }
}
