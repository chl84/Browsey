//! GIO objects stay on GTK's main thread; worker threads only read snapshots.
use gio::{glib, prelude::*};
use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Mutex, time::Duration};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Device {
    pub label: String,
    pub uri: String,
    pub mounted_path: Option<String>,
}

impl Device {
    pub(crate) fn mount_info(&self) -> crate::commands::fs::MountInfo {
        crate::commands::fs::MountInfo {
            label: self.label.clone(),
            path: self.mounted_path.as_ref().unwrap_or(&self.uri).clone(),
            fs: "mtp".into(),
            removable: true,
        }
    }
}

static DEVICES: Mutex<Option<Vec<Device>>> = Mutex::new(None);
thread_local! {
    static WATCH: RefCell<Option<Watch>> = const { RefCell::new(None) };
    static PENDING: RefCell<HashMap<String, gio::Cancellable>> = RefCell::new(HashMap::new());
}

struct Watch {
    monitor: gio::VolumeMonitor,
    signals: Vec<glib::SignalHandlerId>,
}

pub(crate) fn devices() -> Option<Vec<Device>> {
    DEVICES.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

fn volume_uri(volume: &gio::Volume) -> Option<String> {
    let uri = volume.activation_root()?.uri().to_string();
    uri.starts_with("mtp://").then_some(uri)
}

fn mounted_path(volume: &gio::Volume) -> Option<String> {
    volume
        .get_mount()?
        .default_location()
        .path()
        .filter(|path| path.is_absolute())
        .map(|path| path.to_string_lossy().into_owned())
}

fn collect_devices(monitor: &gio::VolumeMonitor) -> Vec<Device> {
    let mut result: Vec<_> = monitor
        .volumes()
        .into_iter()
        .filter_map(|volume| {
            Some(Device {
                uri: volume_uri(&volume)?,
                label: volume.name().to_string(),
                mounted_path: mounted_path(&volume),
            })
        })
        .collect();
    normalize_devices(&mut result);
    result
}

fn normalize_devices(devices: &mut Vec<Device>) {
    devices.sort_by(|a, b| a.uri.cmp(&b.uri));
    devices.dedup_by(|a, b| a.uri == b.uri);
}

fn publish(monitor: &gio::VolumeMonitor, app: &tauri::AppHandle) {
    if crate::runtime_lifecycle::is_shutting_down(app) {
        return;
    }
    let next = collect_devices(monitor);
    let changed = {
        let mut current = DEVICES.lock().unwrap_or_else(|e| e.into_inner());
        if current.as_ref() == Some(&next) {
            false
        } else {
            tracing::debug!(
                devices = next.len(),
                mounted = next.iter().filter(|d| d.mounted_path.is_some()).count(),
                "MTP devices refreshed"
            );
            *current = Some(next);
            true
        }
    };
    if changed {
        crate::runtime_lifecycle::emit_if_running(app, "volumes-changed", ());
    }
}

/// Called from Tauri setup on the GTK main thread, never from a scan worker.
pub(crate) fn start(app: tauri::AppHandle) {
    WATCH.with(|slot| {
        if slot.borrow().is_some() {
            return;
        }
        let monitor = gio::VolumeMonitor::get();
        let mut signals = Vec::new();
        macro_rules! watch {
            ($connect:ident) => {{
                let app = app.clone();
                signals.push(monitor.$connect(move |monitor, _| publish(monitor, &app)));
            }};
        }
        watch!(connect_volume_added);
        watch!(connect_volume_removed);
        watch!(connect_volume_changed);
        watch!(connect_mount_added);
        watch!(connect_mount_removed);
        watch!(connect_mount_changed);
        publish(&monitor, &app);
        *slot.borrow_mut() = Some(Watch { monitor, signals });
    });
}

/// Called on the same main thread during application exit.
pub(crate) fn stop() {
    WATCH.with(|slot| {
        if let Some(watch) = slot.borrow_mut().take() {
            for signal in watch.signals {
                watch.monitor.disconnect(signal);
            }
        }
    });
    let pending = PENDING.with(|slot| std::mem::take(&mut *slot.borrow_mut()));
    for cancellable in pending.into_values() {
        cancellable.cancel();
    }
    *DEVICES.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

fn same_root(request: &str, root: &str) -> bool {
    request.trim_end_matches('/') == root.trim_end_matches('/')
}

fn unavailable_path() -> String {
    "The phone is mounted, but its local GVFS path is unavailable. Try opening it again.".into()
}

fn connected_path(monitor: &gio::VolumeMonitor, volume: &gio::Volume) -> Result<String, String> {
    if !monitor.volumes().contains(volume) {
        return Err("Phone disconnected while connecting. Reconnect it and try again.".into());
    }
    mounted_path(volume).ok_or_else(unavailable_path)
}

type MountReply = tokio::sync::oneshot::Sender<Result<String, String>>;

fn mount_on_main_thread(uri: String, app: tauri::AppHandle, sender: MountReply) {
    if crate::runtime_lifecycle::is_shutting_down(&app) {
        let _ = sender.send(Err("Browsey is shutting down.".into()));
        return;
    }
    let monitor = gio::VolumeMonitor::get();
    let volume = monitor
        .volumes()
        .into_iter()
        .find(|volume| volume_uri(volume).is_some_and(|root| same_root(&uri, &root)));
    let Some(volume) = volume else {
        let _ = sender.send(Err(
            "Phone disconnected or unavailable. Reconnect it, unlock it and select File transfer (MTP).".into(),
        ));
        return;
    };
    let key = volume_uri(&volume).expect("matched MTP volume");
    if volume.get_mount().is_some() {
        publish(&monitor, &app);
        let _ = sender.send(connected_path(&monitor, &volume));
        return;
    }

    let cancellable = gio::Cancellable::new();
    let busy = PENDING.with(|pending| {
        let mut pending = pending.borrow_mut();
        if pending.contains_key(&key) {
            true
        } else {
            pending.insert(key.clone(), cancellable.clone());
            false
        }
    });
    if busy {
        let _ = sender.send(Err("The phone is already connecting. Please wait.".into()));
        return;
    }

    // The deadline replies once and requests GIO cancellation. Keep the pending
    // guard until the callback, so a late backend reply cannot overlap a retry.
    let reply = Rc::new(RefCell::new(Some(sender)));
    let timeout_reply = reply.clone();
    let cancel_on_timeout = cancellable.clone();
    let timeout = glib::timeout_add_local_once(Duration::from_secs(30), move || {
        if let Some(sender) = timeout_reply.borrow_mut().take() {
            let _ = sender.send(Err(
                "Phone connection timed out. Unlock the phone and allow File transfer (MTP), then try again.".into(),
            ));
        }
        cancel_on_timeout.cancel();
    });
    let mounted_volume = volume.clone();
    volume.mount(
        gio::MountMountFlags::NONE,
        gio::MountOperation::NONE,
        Some(&cancellable),
        move |result| {
            PENDING.with(|pending| {
                pending.borrow_mut().remove(&key);
            });
            publish(&monitor, &app);
            if let Some(sender) = reply.borrow_mut().take() {
                timeout.remove();
                let result = match result {
                    Ok(()) => connected_path(&monitor, &mounted_volume),
                    Err(error) if error.matches(gio::IOErrorEnum::AlreadyMounted) => {
                        connected_path(&monitor, &mounted_volume)
                    }
                    Err(error) => Err(format!(
                        "Could not connect to the phone. Unlock it and allow File transfer (MTP). {error}"
                    )),
                };
                let _ = sender.send(result);
            }
        },
    );
}

pub(crate) async fn mount(uri: String, app: tauri::AppHandle) -> Result<String, String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let callback_app = app.clone();
    app.run_on_main_thread(move || mount_on_main_thread(uri, callback_app, sender))
        .map_err(|error| format!("Could not schedule phone connection: {error}"))?;
    receiver
        .await
        .map_err(|_| "Phone connection was interrupted.".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_the_exact_phone_root_not_another_phone_or_child() {
        assert!(same_root("mtp://Phone_A", "mtp://Phone_A/"));
        assert!(!same_root("mtp://Phone_B/", "mtp://Phone_A/"));
        assert!(!same_root("mtp://Phone_A/Internal/", "mtp://Phone_A/"));
        assert!(!same_root("mtp://phone_a/", "mtp://Phone_A/"));
    }

    #[test]
    fn phone_changes_from_mountable_uri_to_exact_local_path() {
        let mut phone = Device {
            label: "Phone".into(),
            uri: "mtp://Phone_A/".into(),
            mounted_path: None,
        };
        assert_eq!(phone.mount_info().path, "mtp://Phone_A/");
        phone.mounted_path = Some("/run/user/1000/gvfs/mtp:host=Phone_A".into());
        assert_eq!(
            phone.mount_info().path,
            "/run/user/1000/gvfs/mtp:host=Phone_A"
        );
        assert_eq!(phone.mount_info().fs, "mtp");
        phone.mounted_path = None;
        assert_eq!(phone.mount_info().path, "mtp://Phone_A/");
    }

    #[test]
    fn snapshot_deduplication_preserves_distinct_phones_with_the_same_label() {
        let a = Device {
            label: "Android".into(),
            uri: "mtp://A/".into(),
            mounted_path: None,
        };
        let b = Device {
            label: "Android".into(),
            uri: "mtp://B/".into(),
            mounted_path: None,
        };
        let mut devices = vec![b.clone(), a.clone(), b.clone()];
        normalize_devices(&mut devices);
        assert_eq!(devices, vec![a, b]);
    }
}
