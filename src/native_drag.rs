//! Export the existing WebKit drag as native files without starting a second drag.
//!
//! WebKitGTK sanitizes DOM text/uri-list as a single URL, concatenating multiline
//! selections. The frontend supplies one URL envelope; this source-only GTK hook
//! substitutes a correctly encoded URI list when a native destination requests it.
use std::{cell::RefCell, path::Path, rc::Rc};

use gtk::prelude::*;
use tauri::{Runtime, WebviewWindow};
use url::Url;

mod portal;

pub const INIT_SCRIPT: &str =
    "Object.defineProperty(window, '__BROWSEY_FILE_DRAG_BRIDGE__', { value: true });";
const PREFIX: &str = "browsey-drag://local/";

fn file_uris(envelope: &str) -> Option<Vec<String>> {
    let url = Url::parse(envelope).ok()?;
    if url.scheme() != "browsey-drag"
        || url.host_str() != Some("local")
        || url.path() != "/"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    let pairs = url.query_pairs().collect::<Vec<_>>();
    if pairs.len() != 1 || pairs[0].0 != "paths" {
        return None;
    }
    let paths: Vec<String> = serde_json::from_str(&pairs[0].1).ok()?;
    if paths.is_empty() {
        return None;
    }
    paths
        .into_iter()
        .map(|path| {
            if !Path::new(&path).is_absolute() || path.contains('\0') {
                return None;
            }
            Url::from_file_path(path).ok().map(String::from)
        })
        .collect()
}

fn rewrite_file_data(data: &gtk::SelectionData, transfer: &RefCell<Option<portal::Transfer>>) {
    let uris = data.uris();
    if uris.len() != 1 || !uris[0].starts_with(PREFIX) {
        return;
    }
    if let Some(files) = file_uris(&uris[0]) {
        if portal::is_target(data.target().name().as_str()) {
            let mut slot = transfer.borrow_mut();
            if slot.is_none() {
                match portal::Transfer::register(&files) {
                    Ok(active) => *slot = Some(active),
                    Err(err) => {
                        data.set(&data.target(), 8, &[]);
                        tracing::warn!("Could not export native drag through file portal: {err}");
                        return;
                    }
                }
            }
            if let Some(active) = slot.as_ref() {
                data.set(&data.target(), 8, active.key().as_bytes());
            }
            return;
        }
        let refs = files.iter().map(String::as_str).collect::<Vec<_>>();
        if !data.set_uris(&refs) {
            tracing::warn!("Could not export native drag URI list");
        }
    } else {
        // Never expose a malformed envelope as a real file selection.
        data.set(&data.target(), 8, &[]);
        tracing::warn!("Rejected invalid native file drag payload");
    }
}

pub fn install<R: Runtime>(window: &WebviewWindow<R>) -> tauri::Result<()> {
    window.with_webview(|platform| {
        let transfer = Rc::new(RefCell::new(None::<portal::Transfer>));
        let begin_transfer = transfer.clone();
        platform.inner().connect_drag_begin(move |_, _| {
            begin_transfer.borrow_mut().take();
        });
        let end_transfer = transfer.clone();
        platform.inner().connect_drag_end(move |_, _| {
            end_transfer.borrow_mut().take();
        });
        let destroy_transfer = transfer.clone();
        platform.inner().connect_destroy(move |_| {
            destroy_transfer.borrow_mut().take();
        });
        // Callback destruction also drops state; no Tauri handles are involved.
        // Run AFTER WebKit's handlers, including ones it installs lazily. Rewrite
        // the borrowed selection before GTK delivers it to the destination. This closure
        // owns no Tauri handle or IPC Channel: destruction cannot evaluate JS or
        // re-enter the window registry (the previous window-close crash).
        platform
            .inner()
            .connect_local("drag-data-get", true, move |values| {
                if let Ok(data) = values[2].get::<&gtk::SelectionData>() {
                    rewrite_file_data(data, &transfer);
                }
                None
            });
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(paths: &[&str]) -> String {
        let mut url = Url::parse(PREFIX).unwrap();
        url.query_pairs_mut()
            .append_pair("paths", &serde_json::to_string(paths).unwrap());
        url.into()
    }

    #[test]
    fn encodes_multiple_files_without_losing_path_boundaries() {
        let paths = [
            "/tmp/first photo.jpg",
            "/tmp/æ #?%+\nsecond.jpg",
            "/tmp/a\\b",
        ];
        let uris = file_uris(&envelope(&paths)).unwrap();
        assert_eq!(uris.len(), paths.len());
        for (uri, path) in uris.iter().zip(paths) {
            assert!(!uri.contains(['\r', '\n']));
            assert_eq!(
                Url::parse(uri).unwrap().to_file_path().unwrap(),
                Path::new(path)
            );
        }
    }

    #[test]
    fn rejects_empty_relative_cloud_and_nul_selections_atomically() {
        for paths in [
            vec![],
            vec!["relative"],
            vec!["/tmp/good", "rclone://remote/file"],
            vec!["/tmp/bad\0file"],
        ] {
            assert!(file_uris(&envelope(&paths)).is_none());
        }
    }

    #[test]
    fn rejects_malformed_or_ambiguous_envelopes() {
        for value in [
            "file:///tmp/file",
            "browsey-drag://local/?paths=bad",
            "browsey-drag://local/?paths=[]&paths=[]",
            "browsey-drag://other/?paths=[]",
            "browsey-drag://local/?paths=[]#fragment",
        ] {
            assert!(file_uris(value).is_none());
        }
    }

    #[test]
    #[ignore = "requires a graphical session and an opt-in native input driver; see docs/testing-native-drag.md"]
    #[allow(deprecated)] // Bounded event pump for deterministic GUI integration testing.
    fn webkit_file_export_and_window_teardown() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        };
        use std::time::{Duration, Instant};

        let backend =
            std::env::var("BROWSEY_TEST_NAUTILUS_BACKEND").unwrap_or_else(|_| "x11".into());
        assert_eq!(
            std::env::var("GDK_BACKEND").as_deref(),
            Ok(backend.as_str())
        );
        let mut app = tauri::Builder::default()
            .any_thread()
            .build(tauri::generate_context!())
            .unwrap();
        let nautilus_mode = std::env::var("BROWSEY_TEST_NAUTILUS_MODE").ok();
        let fixture_root = format!(
            "/tmp/browsey-native-acceptance-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let paths = if nautilus_mode.is_some() {
            [
                format!("{fixture_root}/first photo.txt"),
                format!("{fixture_root}/æ #?%+ second.txt"),
            ]
        } else {
            [
                "/tmp/first photo.jpg".into(),
                "/tmp/æ #?%+\nsecond.jpg".into(),
            ]
        };
        let payload = serde_json::to_string(&envelope(
            &paths.iter().map(String::as_str).collect::<Vec<_>>(),
        ))
        .unwrap();
        // Frontend tests cover modifier -> effectAllowed mapping. This native
        // test isolates the advertised action and actual receiver/file results
        // from compositor-specific synthetic keyboard handling.
        let effect = match nautilus_mode.as_deref() {
            Some("copy") => "copy",
            Some("move") => "move",
            _ => "copyMove",
        };
        let html = format!(
            r#"<!doctype html><html><body style="margin:0"><div draggable="true" style="height:100vh;background:#ace" ondragstart='event.dataTransfer.effectAllowed="{effect}";event.dataTransfer.setData("text/uri-list",{payload})'>Browsey native file export test</div></body></html>"#
        );
        let source = tauri::WebviewWindowBuilder::new(
            &app,
            "native-drag-regression",
            tauri::WebviewUrl::External(Url::parse("about:blank").unwrap()),
        )
        .title("Browsey drag source test")
        .inner_size(500.0, 350.0)
        .decorations(false)
        .build()
        .unwrap();
        install(&source).unwrap();
        let page_ready = Arc::new(AtomicBool::new(false));
        let page_ready_event = page_ready.clone();
        source
            .with_webview(move |platform| {
                use webkit2gtk::WebViewExt;
                platform.inner().connect_load_changed(move |_, event| {
                    if event == webkit2gtk::LoadEvent::Finished {
                        page_ready_event.store(true, Ordering::SeqCst);
                    }
                });
                platform.inner().connect_drag_begin(|_, context| {
                    eprintln!("TEST drag begin: {:?}", context.actions())
                });
                platform.inner().connect_drag_end(|_, context| {
                    eprintln!("TEST drag end: {:?}", context.selected_action())
                });
                platform.inner().load_html(&html, None);
            })
            .unwrap();
        let closed = Arc::new(AtomicBool::new(false));
        let closed_event = closed.clone();
        source.on_window_event(move |event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                closed_event.store(true, Ordering::SeqCst);
            }
        });
        let dest = gtk::Window::new(gtk::WindowType::Toplevel);
        dest.set_title("Browsey native URI receiver test");
        dest.set_default_size(500, 350);
        let label = gtk::Label::new(Some("Native URI receiver (test only)"));
        dest.add(&label);
        label.drag_dest_set(
            gtk::DestDefaults::ALL,
            &[gtk::TargetEntry::new(
                "text/uri-list",
                gtk::TargetFlags::empty(),
                0,
            )],
            gtk::gdk::DragAction::COPY | gtk::gdk::DragAction::MOVE,
        );
        let received = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
        let received_event = received.clone();
        label.connect_drag_data_received(move |_, context, _, _, data, _, time| {
            received_event
                .lock()
                .unwrap()
                .push(data.uris().iter().map(ToString::to_string).collect());
            // The fixture is a URI protocol test, not an actual filesystem copy.
            context.drag_finish(true, false, time);
        });
        if nautilus_mode.is_none() {
            dest.show_all();
        }
        let began = Instant::now();
        let mut driver = None;
        let mut closing = false;
        while !closed.load(Ordering::SeqCst) && began.elapsed() < Duration::from_secs(30) {
            app.run_iteration(|_, event| {
                if let tauri::RunEvent::ExitRequested { api, .. } = event {
                    api.prevent_exit();
                }
            });
            if driver.is_none()
                && page_ready.load(Ordering::SeqCst)
                && began.elapsed() > Duration::from_secs(2)
            {
                let source_gtk = source.gtk_window().unwrap();
                let (_, sx, sy) = source_gtk.window().unwrap().origin();
                let mut command = std::process::Command::new("python3");
                if let Some(mode) = &nautilus_mode {
                    command
                        .arg(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/tests/support/native_drag_nautilus.py"
                        ))
                        .args([
                            (sx + 100).to_string(),
                            (sy + 100).to_string(),
                            fixture_root.clone(),
                            mode.clone(),
                        ]);
                } else {
                    let (_, dx, dy) = dest.window().unwrap().origin();
                    command
                        .arg(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/tests/support/native_drag_pointer.py"
                        ))
                        .args([
                            (sx + 100).to_string(),
                            (sy + 100).to_string(),
                            (dx + 150).to_string(),
                            (dy + 150).to_string(),
                        ]);
                }
                driver = Some(command.spawn().unwrap());
            }
            let nautilus_finished = nautilus_mode.is_some()
                && driver
                    .as_mut()
                    .is_some_and(|child| child.try_wait().unwrap().is_some());
            if !closing && (!received.lock().unwrap().is_empty() || nautilus_finished) {
                source.close().unwrap();
                closing = true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        if let Some(mut driver) = driver {
            assert!(driver.wait().unwrap().success());
        }
        dest.close();
        app.cleanup_before_exit();
        assert!(
            closed.load(Ordering::SeqCst),
            "source window did not close cleanly after drag"
        );
        if nautilus_mode.is_some() {
            return;
        }
        let received = received.lock().unwrap();
        assert_eq!(
            received.len(),
            1,
            "the real WebKit drag did not reach the native receiver"
        );
        let actual = received[0]
            .iter()
            .map(|uri| Url::parse(uri).unwrap().to_file_path().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(actual, paths.map(std::path::PathBuf::from));
        assert!(
            closed.load(Ordering::SeqCst),
            "source window did not close cleanly after drag"
        );
    }
}
