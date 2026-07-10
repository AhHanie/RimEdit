use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PendingFileEvent {
    Changed,
    Deleted,
}

pub(crate) struct DebouncedIndexEvents {
    delay: Duration,
    pending: BTreeMap<(String, String), (PendingFileEvent, Instant)>,
}

impl DebouncedIndexEvents {
    pub(crate) fn new(delay: Duration) -> Self {
        Self {
            delay,
            pending: BTreeMap::new(),
        }
    }

    pub(crate) fn record_change(
        &mut self,
        location_id: String,
        relative_path: String,
        at: Instant,
    ) {
        let key = (location_id, relative_path);
        // Delete wins: don't overwrite a pending delete with a change
        if let Some((PendingFileEvent::Deleted, _)) = self.pending.get(&key) {
            return;
        }
        self.pending.insert(key, (PendingFileEvent::Changed, at));
    }

    pub(crate) fn record_delete(
        &mut self,
        location_id: String,
        relative_path: String,
        at: Instant,
    ) {
        // Delete always wins over any pending change
        self.pending.insert(
            (location_id, relative_path),
            (PendingFileEvent::Deleted, at),
        );
    }

    /// Drain all events whose deadline `(event_time + delay) <= now`.
    pub(crate) fn drain_ready(&mut self, now: Instant) -> Vec<(String, String, PendingFileEvent)> {
        let mut ready = Vec::new();
        let mut remaining = BTreeMap::new();
        for ((loc, path), (event, at)) in std::mem::take(&mut self.pending) {
            if at + self.delay <= now {
                ready.push((loc, path, event));
            } else {
                remaining.insert((loc, path), (event, at));
            }
        }
        self.pending = remaining;
        ready
    }

    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        self.pending.values().map(|(_, at)| *at + self.delay).min()
    }

    #[allow(dead_code)]
    pub(crate) fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn debouncer() -> DebouncedIndexEvents {
        DebouncedIndexEvents::new(Duration::from_millis(300))
    }

    #[test]
    fn debounce_coalesces_repeated_changes_to_same_file() {
        let mut d = debouncer();
        let t0 = Instant::now();
        for _ in 0..5 {
            d.record_change("loc".into(), "Defs/a.xml".into(), t0);
        }
        let ready = d.drain_ready(t0 + Duration::from_millis(301));
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].0, "loc");
        assert_eq!(ready[0].1, "Defs/a.xml");
        assert_eq!(ready[0].2, PendingFileEvent::Changed);
    }

    #[test]
    fn delete_wins_over_change() {
        let mut d = debouncer();
        let t0 = Instant::now();
        d.record_change("loc".into(), "Defs/a.xml".into(), t0);
        d.record_delete("loc".into(), "Defs/a.xml".into(), t0);
        let ready = d.drain_ready(t0 + Duration::from_millis(301));
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].2, PendingFileEvent::Deleted);
    }

    #[test]
    fn change_does_not_overwrite_pending_delete() {
        let mut d = debouncer();
        let t0 = Instant::now();
        d.record_delete("loc".into(), "Defs/a.xml".into(), t0);
        d.record_change("loc".into(), "Defs/a.xml".into(), t0);
        let ready = d.drain_ready(t0 + Duration::from_millis(301));
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].2, PendingFileEvent::Deleted);
    }

    #[test]
    fn events_not_ready_before_delay() {
        let mut d = debouncer();
        let t0 = Instant::now();
        d.record_change("loc".into(), "Defs/a.xml".into(), t0);
        // Drain before delay expires
        let ready = d.drain_ready(t0 + Duration::from_millis(299));
        assert!(ready.is_empty());
        // Drain after delay expires
        let ready = d.drain_ready(t0 + Duration::from_millis(301));
        assert_eq!(ready.len(), 1);
    }

    #[test]
    fn next_deadline_returns_earliest() {
        let mut d = debouncer();
        let t0 = Instant::now();
        let t1 = t0 + Duration::from_millis(100);
        d.record_change("loc".into(), "Defs/a.xml".into(), t0);
        d.record_change("loc".into(), "Defs/b.xml".into(), t1);
        let deadline = d.next_deadline().unwrap();
        assert_eq!(deadline, t0 + Duration::from_millis(300));
    }

    #[test]
    fn is_empty_after_drain() {
        let mut d = debouncer();
        let t0 = Instant::now();
        d.record_change("loc".into(), "Defs/a.xml".into(), t0);
        assert!(!d.is_empty());
        d.drain_ready(t0 + Duration::from_millis(400));
        assert!(d.is_empty());
    }
}
