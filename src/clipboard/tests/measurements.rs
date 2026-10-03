use super::*;
use crate::fs_utils::copy_test_hooks::{Phase, Scope};
use crate::performance_fixture::{report, Fixture};
use std::sync::Arc;
use std::time::Instant;

#[test]
#[ignore = "Five real disposable copies with fixture-scoped 20 ms I/O delay"]
fn controlled_slow_copy_cancellation() {
    let fixture = Fixture::new("slow-copy");
    let source = fixture.0.join("source.bin");
    let data = vec![0x56; 1024 * 1024];
    fs::write(&source, &data).unwrap();
    let mut samples = Vec::new();
    for sample in 0..5 {
        let target = fixture.0.join(format!("target-{sample}.bin"));
        let flag = Arc::new(AtomicBool::new(false));
        let hook_flag = flag.clone();
        let requested = std::rc::Rc::new(std::cell::Cell::new(None));
        let hook_requested = requested.clone();
        let scope = Scope::new(move |_, _, phase, bytes| {
            if phase == Phase::Read && bytes == 65536 {
                hook_requested.set(Some(Instant::now()));
                hook_flag.store(true, Ordering::Relaxed);
                // One blocked read boundary. Never changes global I/O/network state.
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(())
        });
        let result = copy_file_best_effort(&source, &target, None, None, Some(&flag), None);
        let elapsed = requested.get().unwrap().elapsed().as_secs_f64() * 1000.0;
        drop(scope);
        assert_eq!(result.unwrap_err().code(), ClipboardErrorCode::Cancelled);
        assert_eq!(fs::read(&source).unwrap(), data);
        let partial = fs::read(&target).unwrap();
        assert!(partial.len() < data.len());
        assert_eq!(partial, data[..partial.len()]);
        samples.push(elapsed);
    }
    report(
        "copy-controlled-slow-cancellation",
        &samples,
        serde_json::json!({
            "injectedBlockingMs": 20, "sourceBytes": data.len(),
            "sourcePreserved": true, "uncertainOutputsRetained": true,
            "scope": "fixture-scoped blocked read; latency includes the blocked syscall boundary",
        }),
    );
}
