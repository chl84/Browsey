use super::*;
use crate::performance_fixture::Fixture;
use std::{
    fs,
    sync::atomic::{AtomicBool, Ordering},
};

#[test]
fn real_gio_copy_reports_raw_bytes_and_preserves_contents_in_both_directions() {
    let fixture = Fixture::new("gio-byte-progress");
    let network = fixture.0.join("gvfs/server");
    let local = fixture.0.join("local");
    fs::create_dir_all(&network).unwrap();
    fs::create_dir(&local).unwrap();
    let data = vec![0x56; 4 * 1024 * 1024 + 137];
    fs::write(network.join("source.bin"), &data).unwrap();
    for (source, destination) in [
        (network.join("source.bin"), local.join("copy.bin")),
        (local.join("copy.bin"), network.join("round-trip.bin")),
    ] {
        let mut events = Vec::new();
        let bytes = copy(
            &source,
            &destination,
            None,
            || false,
            |event| events.push(event),
        )
        .unwrap();
        assert_eq!(bytes, data.len() as u64);
        assert_eq!(fs::read(&source).unwrap(), data);
        assert_eq!(fs::read(&destination).unwrap(), data);
        assert_eq!(events.first().unwrap().bytes, 0);
        assert!(events.iter().all(|event| event.total == data.len() as u64));
        assert_eq!(events.last().unwrap().bytes, data.len() as u64);
        assert!(events.last().unwrap().finished);
        assert!(events.windows(2).all(|pair| pair[0].bytes <= pair[1].bytes));
    }
}

#[test]
fn gio_copy_never_overwrites_an_existing_or_racing_destination() {
    let fixture = Fixture::new("gio-no-overwrite");
    let source = fixture.0.join("source");
    fs::write(&source, b"source").unwrap();
    for race in [false, true] {
        let target = fixture.0.join(if race { "racing" } else { "existing" });
        if !race {
            fs::write(&target, b"foreign").unwrap();
        }
        let error = copy(
            &source,
            &target,
            None,
            || false,
            |event| {
                if race && event.bytes == 0 {
                    fs::write(&target, b"foreign").unwrap();
                }
            },
        )
        .unwrap_err();
        assert_eq!(error.code(), ClipboardErrorCode::DestinationExists);
        assert_eq!(fs::read(&target).unwrap(), b"foreign");
        assert_eq!(fs::read(&source).unwrap(), b"source");
    }
}

#[test]
fn precancelled_gio_copy_does_not_open_or_create_any_output() {
    let fixture = Fixture::new("gio-precancelled");
    let source = fixture.0.join("source");
    let target = fixture.0.join("target");
    fs::write(&source, b"keep source").unwrap();
    let error = copy(
        &source,
        &target,
        None,
        || true,
        |_| panic!("No progress before opening"),
    )
    .unwrap_err();
    assert_eq!(error.code(), ClipboardErrorCode::Cancelled);
    assert!(!target.exists());
    assert_eq!(fs::read(source).unwrap(), b"keep source");
}

#[test]
fn cancel_registry_interrupts_blocked_gio_work_without_progress_callbacks() {
    let state = crate::tasks::CancelState::default();
    let guard = state.register("silent-copy-fixture".into()).unwrap();
    let flag = guard.token();
    let (started, ready) = mpsc::channel();
    std::thread::scope(|scope| {
        let cancelling_state = state.clone();
        scope.spawn(move || {
            ready.recv_timeout(Duration::from_secs(3)).unwrap();
            assert!(cancelling_state.cancel("silent-copy-fixture").unwrap());
        });
        with_cancellable(
            || flag.load(Ordering::Relaxed),
            |cancel| {
                let (stopped, wait) = mpsc::channel();
                cancel.connect_cancelled(move |_| {
                    let _ = stopped.send(());
                });
                started.send(()).unwrap();
                // Models a blocked GIO call; no progress/event/text can wake it.
                wait.recv_timeout(Duration::from_secs(3)).unwrap();
                assert!(cancel.is_cancelled());
            },
        );
    });
    drop(guard);
    assert!(!state.cancel("silent-copy-fixture").unwrap());
}

#[test]
fn cancellation_during_gio_progress_is_terminal_and_preserves_source() {
    let fixture = Fixture::new("gio-active-cancel");
    let source = fixture.0.join("source");
    let target = fixture.0.join("target");
    let data = vec![0x42; 8 * 1024 * 1024];
    fs::write(&source, &data).unwrap();
    let flag = AtomicBool::new(false);
    let mut completed = false;
    let error = copy(
        &source,
        &target,
        None,
        || flag.load(Ordering::Relaxed),
        |event| {
            if event.bytes > 0 {
                flag.store(true, Ordering::Relaxed);
            }
            completed |= event.finished;
        },
    )
    .unwrap_err();
    assert_eq!(error.code(), ClipboardErrorCode::Cancelled);
    assert!(!completed);
    assert_eq!(fs::read(&source).unwrap(), data);
    if target.exists() {
        assert!(fs::metadata(&target).unwrap().len() <= data.len() as u64);
    }
}

#[cfg(unix)]
#[test]
fn gio_copy_rejects_symlinks_without_reading_their_targets() {
    let fixture = Fixture::new("gio-symlink");
    let source = fixture.0.join("source");
    let link = fixture.0.join("link");
    let target = fixture.0.join("target");
    fs::write(&source, b"private source").unwrap();
    std::os::unix::fs::symlink(&source, &link).unwrap();
    let error = copy(&link, &target, None, || false, |_| {}).unwrap_err();
    assert_eq!(error.code(), ClipboardErrorCode::SymlinkUnsupported);
    assert!(!target.exists());
}

#[test]
fn gvfs_dispatch_returns_gio_error_without_retrying_the_manual_copy_path() {
    let fixture = Fixture::new("gio-no-retry");
    let path = fixture.0.join("gvfs/missing-source");
    let target = fixture.0.join("target");
    let error =
        crate::clipboard::ops::copy_file_best_effort(&path, &target, None, None, None, None)
            .unwrap_err();
    assert_eq!(error.code(), ClipboardErrorCode::NotFound);
    assert!(error.to_string().contains("Network copy failed"));
    assert!(!target.exists());
}

#[test]
fn native_gio_metadata_totals_count_contents_and_support_cancellation() {
    let fixture = Fixture::new("gio-size");
    fs::create_dir_all(fixture.0.join("empty/nested")).unwrap();
    fs::write(fixture.0.join("a"), [1; 13]).unwrap();
    fs::write(fixture.0.join("empty/b"), [2; 27]).unwrap();
    assert_eq!(estimate_size(&fixture.0, || false).unwrap(), Some(40));
    assert_eq!(
        estimate_size(&fixture.0, || true).unwrap_err().code(),
        ClipboardErrorCode::Cancelled
    );
}
