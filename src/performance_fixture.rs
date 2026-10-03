//! Shared disposable workload support. No personal paths or global cache resets.
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) struct Fixture(pub(crate) PathBuf);
impl Fixture {
    pub(crate) fn new(label: &str) -> Self {
        assert!(label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'));
        let path = std::env::temp_dir().join(format!(
            "browsey-perf-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    pub(crate) fn entries(&self, count: usize, recursive: bool) -> PathBuf {
        assert!(
            (1..=100_000).contains(&count),
            "bounded opt-in fixture size"
        );
        let root = self.0.join(format!("{count}-{recursive}"));
        fs::create_dir(&root).unwrap();
        for index in 0..count {
            let parent = if recursive {
                root.join(format!("group-{:03}", index % 100))
            } else {
                root.clone()
            };
            fs::create_dir_all(&parent).unwrap();
            let tag = if index % 1000 == 0 {
                "needle"
            } else {
                "ordinary"
            };
            fs::write(
                parent.join(format!("item-{index:06}-{tag}.txt")),
                b"fixture\n",
            )
            .unwrap();
        }
        root
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

pub(crate) fn report(label: &str, samples_ms: &[f64], extra: serde_json::Value) {
    assert!(!samples_ms.is_empty());
    let mut sorted = samples_ms.to_vec();
    sorted.sort_by(f64::total_cmp);
    let memory = fs::read_to_string("/proc/self/status").unwrap();
    let memory = memory
        .lines()
        .filter(|line| line.starts_with("VmRSS:") || line.starts_with("VmHWM:"))
        .collect::<Vec<_>>();
    println!(
        "BROWSEY_PERFORMANCE={}",
        serde_json::json!({
            "schema": 1, "label": label, "samplesMs": samples_ms,
            "medianMs": sorted[sorted.len() / 2], "maxMs": sorted[sorted.len() - 1],
            "samples": sorted.len(), "processMemory": memory,
            "optimized": !cfg!(debug_assertions), "extra": extra,
        })
    );
}
