//! Send limits kept per key (a session or a user), on the service's own clock.

use melior_domain::{ChatError, SendLimit};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Instant;

/// One [`SendLimit`] per key, each made by `make` on its key's first send.
pub(super) struct KeyedLimits<K> {
    make: fn() -> SendLimit,
    limits: Mutex<HashMap<K, SendLimit>>,
    started: Instant,
}

impl<K: Hash + Eq + Clone> KeyedLimits<K> {
    pub(super) fn new(make: fn() -> SendLimit) -> Self {
        Self { make, limits: Mutex::new(HashMap::new()), started: Instant::now() }
    }

    /// Records a send for `key` now, or refuses it when its window is full.
    pub(super) fn record(&self, key: &K) -> Result<(), ChatError> {
        let now_ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.lock().entry(key.clone()).or_insert_with(self.make).record(now_ms)
    }

    pub(super) fn forget(&self, key: &K) {
        self.lock().remove(key);
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<K, SendLimit>> {
        self.limits.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
