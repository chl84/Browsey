//! Adapt operation-scoped transport statistics to Browsey's existing activity events.
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct StatsObserver(Arc<dyn Fn(&Value) + Send + Sync>);

impl std::fmt::Debug for StatsObserver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StatsObserver")
    }
}

impl StatsObserver {
    pub(crate) fn new(callback: impl Fn(&Value) + Send + Sync + 'static) -> Self {
        Self(Arc::new(callback))
    }

    pub(crate) fn observe(&self, stats: &Value) {
        (self.0)(stats);
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(untagged)]
pub(crate) enum ProgressPayload {
    Bytes {
        bytes: u64,
        total: u64,
        finished: bool,
    },
    Items {
        unit: &'static str,
        items: u64,
        total: u64,
        finished: bool,
    },
}

pub(crate) fn stats_payload(stats: &Value) -> Option<ProgressPayload> {
    let count = |key| stats.get(key).and_then(Value::as_u64).unwrap_or(0);
    // Never add server-side byte counters to bytes: they can describe the same
    // work. When a server-side operation exposes no bytes, use completed items.
    let bytes = count("bytes");
    let total = count("totalBytes");
    let server_items = count("serverSideCopies").saturating_add(count("serverSideMoves"));
    if bytes == 0 && server_items > 0 {
        return Some(ProgressPayload::Items {
            unit: "items",
            items: count("transfers").max(count("renames")).max(server_items),
            total: count("totalTransfers"),
            finished: false,
        });
    }
    if total > 0 || bytes > 0 {
        return Some(ProgressPayload::Bytes {
            bytes,
            total,
            finished: false,
        });
    }
    let items = count("transfers")
        .max(count("renames"))
        .max(count("serverSideCopies").saturating_add(count("serverSideMoves")));
    let total = count("totalTransfers");
    if total > 0 || items > 0 {
        Some(ProgressPayload::Items {
            unit: "items",
            items,
            total,
            finished: false,
        })
    } else {
        None
    }
}

pub(crate) fn observer(app: &tauri::AppHandle, event: Option<&str>) -> Option<StatsObserver> {
    let event = event?.to_owned();
    let app = app.clone();
    Some(StatsObserver::new(move |stats| {
        if let Some(payload) = stats_payload(stats) {
            crate::runtime_lifecycle::emit_if_running(&app, &event, payload);
        }
    }))
}

pub(crate) fn items(app: &tauri::AppHandle, event: Option<&str>, done: u64, total: u64) {
    if let Some(event) = event {
        crate::runtime_lifecycle::emit_if_running(
            app,
            event,
            ProgressPayload::Items {
                unit: "items",
                items: done,
                total,
                finished: false,
            },
        );
    }
}

pub(crate) fn phase(app: &tauri::AppHandle, event: Option<&str>, label: &str) {
    if let Some(event) = event {
        crate::runtime_lifecycle::emit_if_running(
            app,
            event,
            serde_json::json!({
                "bytes": 0, "total": 0, "finished": false, "phase": label,
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bytes_unknown_totals_empty_files_and_server_side_work_keep_their_units() {
        assert_eq!(
            stats_payload(&json!({"bytes": 20, "totalBytes": 100,
            "serverSideCopyBytes": 20})),
            Some(ProgressPayload::Bytes {
                bytes: 20,
                total: 100,
                finished: false,
            })
        );
        assert_eq!(
            stats_payload(&json!({"bytes": 20})),
            Some(ProgressPayload::Bytes {
                bytes: 20,
                total: 0,
                finished: false,
            })
        );
        assert_eq!(
            stats_payload(&json!({"transfers": 2, "serverSideCopies": 2,
            "totalTransfers": 3})),
            Some(ProgressPayload::Items {
                unit: "items",
                items: 2,
                total: 3,
                finished: false,
            })
        );
        assert_eq!(stats_payload(&json!({"errors": 4, "listed": 99})), None);
        assert_eq!(
            stats_payload(&json!({"bytes": 0, "totalBytes": 100,
            "serverSideMoves": 2, "totalTransfers": 3})),
            Some(ProgressPayload::Items {
                unit: "items",
                items: 2,
                total: 3,
                finished: false,
            })
        );
        assert_eq!(
            stats_payload(&json!({"bytes": -1, "totalBytes": "100"})),
            None
        );
    }
}
