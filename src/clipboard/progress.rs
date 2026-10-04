//! A paste owns aggregate progress; a child file never finishes the whole task.
use std::cell::Cell;

use super::CopyProgressPayload;

pub(super) struct CopyProgress<'a> {
    app: Option<&'a tauri::AppHandle>,
    event: &'a str,
    total: Option<u64>,
    completed: Cell<u64>,
    observed: Cell<u64>,
}

impl<'a> CopyProgress<'a> {
    pub(super) fn new(
        app: Option<&'a tauri::AppHandle>,
        event: &'a str,
        total: Option<u64>,
    ) -> Self {
        Self {
            app,
            event,
            total,
            completed: Cell::new(0),
            observed: Cell::new(0),
        }
    }

    fn aggregate(&self, payload: CopyProgressPayload) -> CopyProgressPayload {
        let bytes = self
            .completed
            .get()
            .saturating_add(payload.bytes)
            .max(self.observed.get());
        self.observed.set(bytes);
        if payload.finished {
            self.completed.set(bytes);
        }
        CopyProgressPayload {
            bytes,
            total: self.total.unwrap_or(0),
            finished: false,
        }
    }

    pub(super) fn report(&self, payload: CopyProgressPayload) {
        self.emit(self.aggregate(payload));
    }

    pub(super) fn finish(&self) {
        self.emit(CopyProgressPayload {
            bytes: self.total.unwrap_or(self.observed.get()),
            total: self.total.unwrap_or(0),
            finished: true,
        });
    }

    fn emit(&self, payload: CopyProgressPayload) {
        if let Some(app) = self.app {
            let _ = crate::runtime_lifecycle::emit_if_running(app, self.event, payload);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn byte_progress_is_monotonic_across_files_and_child_completion_is_not_task_completion() {
        let batch = CopyProgress::new(None, "fixture", Some(300));
        let report = |bytes, finished| {
            batch.aggregate(CopyProgressPayload {
                bytes,
                total: 100,
                finished,
            })
        };
        assert_eq!(report(50, false).bytes, 50);
        assert_eq!(report(10, false).bytes, 50);
        assert!(!report(100, true).finished);
        assert_eq!(report(0, false).bytes, 100);
        assert_eq!(report(60, false).bytes, 160);
        let final_child = report(200, true);
        assert_eq!(final_child.bytes, 300);
        assert_eq!(final_child.total, 300);
        assert!(!final_child.finished);
    }

    #[test]
    fn unknown_total_stays_indeterminate_without_using_directory_metadata_as_bytes() {
        let batch = CopyProgress::new(None, "fixture", None);
        let payload = batch.aggregate(CopyProgressPayload {
            bytes: 1024,
            total: 2048,
            finished: false,
        });
        assert_eq!(payload.bytes, 1024);
        assert_eq!(payload.total, 0);
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn directory_copies_aggregate_native_gio_bytes_in_both_fixture_directions() {
        let fixture = crate::performance_fixture::Fixture::new("gio-batch-progress");
        let remote = fixture.0.join("gvfs/server/source");
        let local = fixture.0.join("local-copy");
        std::fs::create_dir_all(remote.join("empty/nested")).unwrap();
        std::fs::write(remote.join("a"), [1; 13]).unwrap();
        std::fs::write(remote.join("empty/b"), [2; 27]).unwrap();
        for (src, dst) in [
            (&remote, local.clone()),
            (&local, fixture.0.join("gvfs/server/round-trip")),
        ] {
            let total = super::super::clipboard_size::estimate_total_size(
                std::slice::from_ref(src),
                || false,
            )
            .unwrap();
            let batch = CopyProgress::new(None, "fixture", total);
            super::super::ops::copy_entry(src, &dst, None, Some(&batch), None).unwrap();
            assert_eq!(batch.observed.get(), 40);
            assert_eq!(batch.completed.get(), 40);
            assert_eq!(total, Some(40));
            assert!(dst.join("empty/nested").is_dir());
            assert_eq!(std::fs::read(dst.join("a")).unwrap(), [1; 13]);
            assert_eq!(std::fs::read(dst.join("empty/b")).unwrap(), [2; 27]);
        }
    }
}
