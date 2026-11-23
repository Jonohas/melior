use super::ChatError;
use std::collections::VecDeque;

/// How many messages one participant may send within a window. A sliding window:
/// the oldest send leaves it as time passes.
#[derive(Debug, Clone)]
pub struct SendLimit {
    max: usize,
    window_ms: u64,
    sent: VecDeque<u64>,
}

impl SendLimit {
    /// Five messages per ten seconds.
    #[must_use]
    pub fn standard() -> Self {
        Self::new(5, 10_000)
    }

    /// Ten image uploads per minute.
    #[must_use]
    pub fn uploads() -> Self {
        Self::new(10, 60_000)
    }

    #[must_use]
    pub fn new(max: usize, window_ms: u64) -> Self {
        Self { max, window_ms, sent: VecDeque::new() }
    }

    /// Records a send at `now_ms`, or refuses it when the window is full. A
    /// refused send is not recorded, so waiting is enough to send again.
    pub fn record(&mut self, now_ms: u64) -> Result<(), ChatError> {
        while self.sent.front().is_some_and(|&at| now_ms.saturating_sub(at) >= self.window_ms) {
            self.sent.pop_front();
        }
        if self.sent.len() >= self.max {
            return Err(ChatError::RateLimited);
        }
        self.sent.push_back(now_ms);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sixth_message_within_the_window_is_refused() {
        let mut limit = SendLimit::standard();
        for second in 0..5 {
            assert_eq!(limit.record(second * 1000), Ok(()));
        }
        assert_eq!(limit.record(5000), Err(ChatError::RateLimited));
    }

    #[test]
    fn sending_is_possible_again_once_the_oldest_message_leaves_the_window() {
        let mut limit = SendLimit::new(2, 1000);
        assert_eq!(limit.record(0), Ok(()));
        assert_eq!(limit.record(500), Ok(()));
        assert_eq!(limit.record(999), Err(ChatError::RateLimited));
        assert_eq!(limit.record(1000), Ok(()));
    }

    #[test]
    fn a_refused_message_is_not_counted() {
        let mut limit = SendLimit::new(1, 1000);
        assert_eq!(limit.record(0), Ok(()));
        assert_eq!(limit.record(10), Err(ChatError::RateLimited));
        assert_eq!(limit.record(1000), Ok(()));
    }

    #[test]
    fn a_clock_that_goes_backwards_does_not_panic() {
        let mut limit = SendLimit::new(1, 1000);
        assert_eq!(limit.record(5000), Ok(()));
        assert_eq!(limit.record(1000), Err(ChatError::RateLimited));
    }
}
