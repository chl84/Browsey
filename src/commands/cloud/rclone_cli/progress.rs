//! Operation-local counters: heartbeats and error logs do not extend a transfer.
use serde_json::Value;
use std::time::{Duration, Instant};

pub(crate) struct TransferActivity {
    last_progress: Instant,
    high_water: [u64; 11],
}

impl TransferActivity {
    pub(crate) fn new(started: Instant) -> Self {
        Self {
            last_progress: started,
            high_water: [0; 11],
        }
    }

    pub(crate) fn observe(&mut self, stats: &Value, now: Instant) {
        let mut advanced = false;
        for (index, name) in [
            "bytes",
            "checks",
            "transfers",
            "renames",
            "deletes",
            "deletedDirs",
            "listed",
            "serverSideCopies",
            "serverSideCopyBytes",
            "serverSideMoves",
            "serverSideMoveBytes",
        ]
        .iter()
        .enumerate()
        {
            if let Some(value) = stats.get(name).and_then(Value::as_u64) {
                if value > self.high_water[index] {
                    self.high_water[index] = value;
                    advanced = true;
                }
            }
        }
        if advanced {
            self.last_progress = now;
        }
    }

    pub(crate) fn idle_for(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.last_progress)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn advancing_work_can_outlive_the_original_deadline() {
        let start = Instant::now();
        let mut activity = TransferActivity::new(start);
        for second in 1..=1000 {
            let now = start + Duration::from_secs(second);
            activity.observe(&json!({"bytes": second}), now);
            assert_eq!(activity.idle_for(now), Duration::ZERO);
        }
        assert_eq!(
            activity.idle_for(start + Duration::from_secs(1300)),
            Duration::from_secs(300)
        );
    }

    #[test]
    fn heartbeats_errors_and_counter_resets_do_not_hide_stalls() {
        let start = Instant::now();
        let mut activity = TransferActivity::new(start);
        activity.observe(
            &json!({"bytes": 100, "transfers": 1}),
            start + Duration::from_secs(1),
        );
        for stats in [
            json!({"bytes":100,"transfers":1,"elapsedTime":400,"errors":12}),
            json!({"bytes":0}),
            json!({"bytes":-1,"checks":"99"}),
            json!({"msg":"still working"}),
        ] {
            activity.observe(&stats, start + Duration::from_secs(400));
        }
        assert_eq!(
            activity.idle_for(start + Duration::from_secs(400)),
            Duration::from_secs(399)
        );
    }

    #[test]
    fn completed_empty_files_and_server_side_work_count_as_progress() {
        let start = Instant::now();
        for name in ["checks", "transfers", "renames", "deletes", "deletedDirs"] {
            let mut activity = TransferActivity::new(start);
            activity.observe(&json!({name:1,"bytes":0}), start + Duration::from_secs(250));
            assert_eq!(
                activity.idle_for(start + Duration::from_secs(300)),
                Duration::from_secs(50)
            );
        }
    }
}
