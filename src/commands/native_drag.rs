//! Copy-only native file export, without a JavaScript completion channel.
use std::path::{Path, PathBuf};

use tauri::{Runtime, WebviewWindow};

use crate::errors::api_error::{ApiError, ApiResult};
use crate::fs_utils::sanitize_path_follow;

fn validate_paths(paths: Vec<String>) -> ApiResult<Vec<PathBuf>> {
    if paths.is_empty() {
        return Err(ApiError::new(
            "native_drag_invalid_path",
            "No files selected",
        ));
    }
    let mut validated = Vec::with_capacity(paths.len());
    for raw in paths {
        let path = Path::new(&raw);
        if !path.is_absolute() {
            return Err(ApiError::new(
                "native_drag_invalid_path",
                "Native drag requires absolute local paths",
            ));
        }
        let metadata = std::fs::symlink_metadata(path).map_err(|error| {
            ApiError::new(
                "native_drag_invalid_path",
                format!("Cannot inspect source: {error}"),
            )
        })?;
        if !metadata.is_file() && !metadata.is_dir() {
            return Err(ApiError::new(
                "native_drag_invalid_path",
                "Only regular files and folders can be exported",
            ));
        }
        let path = sanitize_path_follow(&raw, true)
            .map_err(|error| ApiError::new("native_drag_invalid_path", error.to_string()))?;
        if !validated.contains(&path) {
            validated.push(path);
        }
    }
    Ok(validated)
}

// GTK retains this callback in signal handlers until drag/window teardown.
// A Tauri Channel's destructor evaluates JS, re-entering the runtime while its
// window registry is mutably borrowed during close (SIGABRT / BorrowMutError).
// Keep this a function pointer: no Channel, WebviewWindow or AppHandle captures.
const DRAG_FINISHED: fn(drag::DragResult, drag::CursorPosition) = |_, _| {};

fn start_on_main_thread<R: Runtime>(
    window: &WebviewWindow<R>,
    paths: Vec<PathBuf>,
) -> ApiResult<()> {
    #[cfg(target_os = "linux")]
    let raw_window = window
        .gtk_window()
        .map_err(|error| ApiError::new("native_drag_unavailable", error.to_string()))?;
    #[cfg(not(target_os = "linux"))]
    let raw_window = window.clone();

    drag::start_drag(
        &raw_window,
        drag::DragItem::Files(paths),
        drag::Image::Raw(include_bytes!("../../resources/icons/icon.png").to_vec()),
        DRAG_FINISHED,
        drag::Options {
            mode: drag::DragMode::Copy,
            ..Default::default()
        },
    )
    .map_err(|error| ApiError::new("native_drag_failed", error.to_string()))
}

#[tauri::command]
pub async fn start_native_file_drag<R: Runtime>(
    window: WebviewWindow<R>,
    paths: Vec<String>,
) -> ApiResult<()> {
    // Metadata on removable/GVfs paths must not block the GUI/event loop.
    let paths = tauri::async_runtime::spawn_blocking(move || validate_paths(paths))
        .await
        .map_err(|error| ApiError::new("native_drag_failed", error.to_string()))??;
    let (tx, rx) = tokio::sync::oneshot::channel();
    let drag_window = window.clone();
    window
        .run_on_main_thread(move || {
            // Closing the window/request is allowed; never panic on a lost receiver.
            let _ = tx.send(start_on_main_thread(&drag_window, paths));
        })
        .map_err(|error| ApiError::new("native_drag_unavailable", error.to_string()))?;
    rx.await.map_err(|_| {
        ApiError::new(
            "native_drag_unavailable",
            "Window closed before drag could start",
        )
    })?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_relative_and_remote_sources() {
        for paths in [
            vec![],
            vec!["Cargo.toml".into()],
            vec!["rclone://remote/file".into()],
        ] {
            assert_eq!(
                validate_paths(paths).unwrap_err().code,
                "native_drag_invalid_path"
            );
        }
    }

    #[test]
    fn validates_files_and_folders_and_deduplicates() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let file = root.join("Cargo.toml");
        let paths = vec![
            file.display().to_string(),
            root.display().to_string(),
            file.display().to_string(),
        ];
        assert_eq!(
            validate_paths(paths).unwrap(),
            vec![file.canonicalize().unwrap(), root.canonicalize().unwrap()]
        );
    }

    #[test]
    fn rejects_the_whole_selection_if_a_source_is_missing() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        assert!(validate_paths(vec![
            root.join("Cargo.toml").display().to_string(),
            root.join("missing-native-drag-fixture")
                .display()
                .to_string()
        ])
        .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_special_files() {
        assert!(validate_paths(vec!["/dev/null".into()]).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn rejects_symlinks_without_following_them() {
        assert!(std::fs::symlink_metadata("/proc/self/exe")
            .unwrap()
            .is_symlink());
        assert!(validate_paths(vec!["/proc/self/exe".into()]).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_filesystem_root() {
        assert!(validate_paths(vec!["/".into()]).is_err());
    }

    // Run separately, with a graphical session and one test thread. This exercises
    // real GTK drag signal destruction through Tauri's on_window_close, not mocks.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires a graphical session; opens and closes a native test window"]
    #[allow(deprecated)] // Bounded, throttled pump allows deterministic drag/close timing.
    fn native_drag_window_teardown() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };
        use std::time::{Duration, Instant};

        let mut app = tauri::Builder::default()
            .any_thread()
            .build(tauri::generate_context!())
            .expect("build test application");
        let window = tauri::WebviewWindowBuilder::new(
            &app,
            "native-drag-regression",
            tauri::WebviewUrl::External("about:blank".parse().unwrap()),
        )
        .title("Browsey native drag regression test")
        .inner_size(320.0, 160.0)
        .build()
        .expect("build test window");
        let destroyed = Arc::new(AtomicBool::new(false));
        let destroyed_event = destroyed.clone();
        window.on_window_event(move |event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                destroyed_event.store(true, Ordering::SeqCst);
            }
        });
        let began = Instant::now();
        let mut started = false;
        let mut closing = false;
        while !destroyed.load(Ordering::SeqCst) && began.elapsed() < Duration::from_secs(10) {
            app.run_iteration(|_, event| {
                if let tauri::RunEvent::ExitRequested { api, .. } = event {
                    api.prevent_exit();
                }
            });
            if !started && began.elapsed() > Duration::from_millis(500) {
                start_on_main_thread(
                    &window,
                    vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")],
                )
                .expect("start real GTK drag");
                started = true;
            }
            if started && !closing && began.elapsed() > Duration::from_millis(900) {
                window.close().expect("request window close");
                closing = true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            started && destroyed.load(Ordering::SeqCst),
            "native drag window did not close cleanly"
        );
        app.cleanup_before_exit();
    }
}
