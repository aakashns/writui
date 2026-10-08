//! When to store the post: once typing pauses, and every few seconds
//! during a long stretch of typing, so a crash loses at most that much.

use std::time::{Duration, Instant};

/// Store this long after the last edit...
pub const PAUSE: Duration = Duration::from_secs(1);
/// ...or this long after the first edit that isn't stored yet, if sooner.
pub const MAX_WAIT: Duration = Duration::from_secs(5);
/// After storing fails, try again this much later.
pub const RETRY: Duration = Duration::from_secs(5);

#[derive(Default)]
pub struct Autosave {
    /// The first and last edits since the post was last stored.
    first: Option<Instant>,
    last: Option<Instant>,
    /// Don't try again before this, after a failure.
    retry: Option<Instant>,
}

impl Autosave {
    pub fn edited(&mut self, now: Instant) {
        self.first.get_or_insert(now);
        self.last = Some(now);
    }

    /// When the post should next be stored, if it has unstored edits.
    pub fn due(&self) -> Option<Instant> {
        let due = (self.last? + PAUSE).min(self.first? + MAX_WAIT);
        Some(self.retry.map_or(due, |retry| due.max(retry)))
    }

    pub fn stored(&mut self) {
        *self = Autosave::default();
    }

    pub fn failed(&mut self, now: Instant) {
        self.retry = Some(now + RETRY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(s: f64) -> Duration {
        Duration::from_secs_f64(s)
    }

    #[test]
    fn stores_after_a_pause() {
        let start = Instant::now();
        let mut a = Autosave::default();
        assert_eq!(a.due(), None);
        a.edited(start);
        assert_eq!(a.due(), Some(start + PAUSE));
        a.edited(start + secs(0.5));
        assert_eq!(a.due(), Some(start + secs(1.5)));
        a.stored();
        assert_eq!(a.due(), None);
    }

    #[test]
    fn stores_during_long_typing() {
        let start = Instant::now();
        let mut a = Autosave::default();
        for i in 0..100 {
            a.edited(start + secs(i as f64 * 0.2));
        }
        assert_eq!(a.due(), Some(start + MAX_WAIT));
    }

    #[test]
    fn waits_before_retrying() {
        let start = Instant::now();
        let mut a = Autosave::default();
        a.edited(start);
        a.failed(start + PAUSE);
        assert_eq!(a.due(), Some(start + PAUSE + RETRY));
        a.stored();
        a.edited(start + secs(10.0));
        assert_eq!(a.due(), Some(start + secs(11.0)));
    }
}
