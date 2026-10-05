//! Linux network mounts use GTK's authentication UI, not a non-interactive CLI.
//!
//! GTK owns the password fields and GIO/GVFS owns optional keyring storage and
//! host-key questions. Browsey never reads or serializes the password.

use gio::{glib, prelude::*};
use gtk::prelude::WidgetExt;
use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};
use tauri::Manager;

use super::{
    error::{NetworkError, NetworkErrorCode, NetworkResult},
    saved,
};

#[derive(Debug)]
pub(super) struct MountedNetwork {
    pub uri: String,
    pub path: String,
    pub warning: Option<String>,
}

type Reply = Rc<RefCell<Option<tokio::sync::oneshot::Sender<Result<Option<String>, String>>>>>;

#[derive(Clone)]
struct Pending {
    cancel: gio::Cancellable,
    operation: gtk::MountOperation,
    reply: Reply,
}

impl Pending {
    fn abort(&self, message: &str) {
        if let Some(reply) = self.reply.borrow_mut().take() {
            let _ = reply.send(Err(message.to_string()));
        }
        self.cancel.cancel();
        // Also close any GTK password/host-key dialog currently visible.
        self.operation.emit_by_name::<()>("aborted", &[]);
    }
}

thread_local! {
    static PENDING: RefCell<HashMap<String, Pending>> = RefCell::new(HashMap::new());
}

/// Must run on GTK's main thread, including during application shutdown.
pub(crate) fn stop() {
    let pending = PENDING.with(|slot| std::mem::take(&mut *slot.borrow_mut()));
    for operation in pending.into_values() {
        operation.abort("Browsey is shutting down.");
    }
}

fn finish_reply(reply: &Reply, result: Result<Option<String>, String>) -> bool {
    if let Some(sender) = reply.borrow_mut().take() {
        let _ = sender.send(result);
        true
    } else {
        false
    }
}

fn connection_key(uri: &str) -> String {
    // One pending mount per server/account, including requests to child folders.
    let mut url = url::Url::parse(uri).expect("validated server URI");
    url.set_path("/");
    url.to_string()
}

fn mount_on_main_thread(
    uri: String,
    app: tauri::AppHandle,
    sender: tokio::sync::oneshot::Sender<Result<Option<String>, String>>,
) {
    if crate::runtime_lifecycle::is_shutting_down(&app) {
        let _ = sender.send(Err("Browsey is shutting down.".into()));
        return;
    }
    let key = connection_key(&uri);
    if PENDING.with(|slot| slot.borrow().contains_key(&key)) {
        let _ = sender.send(Err(
            "This server/account is already connecting. Please wait.".into(),
        ));
        return;
    }
    let Some(window) = app.get_webview_window("main") else {
        let _ = sender.send(Err("The Browsey window is unavailable.".into()));
        return;
    };
    let parent = match window.gtk_window() {
        Ok(parent) => parent,
        Err(_) => {
            let _ = sender.send(Err(
                "Could not open the network authentication dialog.".into()
            ));
            return;
        }
    };
    let operation = gtk::MountOperation::new(Some(&parent));
    operation.set_password_save(gio::PasswordSave::Never);
    let cancel = gio::Cancellable::new();
    let reply = Rc::new(RefCell::new(Some(sender)));
    PENDING.with(|slot| {
        slot.borrow_mut().insert(
            key.clone(),
            Pending {
                cancel: cancel.clone(),
                operation: operation.clone(),
                reply: reply.clone(),
            },
        )
    });

    let timeout_key = key.clone();
    let timeout = glib::timeout_add_local_once(Duration::from_secs(120), move || {
        let pending = PENDING.with(|slot| slot.borrow().get(&timeout_key).cloned());
        if let Some(pending) = pending {
            pending.abort(
                "Server connection timed out. Check the address and credentials, then try again.",
            );
        }
    });
    let destroy_cancel = cancel.clone();
    let destroy_operation = operation.clone();
    let destroy_signal = parent.connect_destroy(move |_| {
        destroy_cancel.cancel();
        destroy_operation.emit_by_name::<()>("aborted", &[]);
    });
    let callback_operation = operation.clone();
    gio::File::for_uri(&uri).mount_enclosing_volume(
        gio::MountMountFlags::NONE,
        Some(&operation),
        Some(&cancel),
        move |result| {
            PENDING.with(|slot| {
                slot.borrow_mut().remove(&key);
            });
            parent.disconnect(destroy_signal);
            let result = match result {
                Ok(()) => Ok(callback_operation
                    .username()
                    .map(|username| username.to_string())),
                Err(error) if error.matches(gio::IOErrorEnum::AlreadyMounted) => Ok(None),
                Err(error) if error.matches(gio::IOErrorEnum::Cancelled) => {
                    Err("Connection cancelled.".into())
                }
                Err(error) => Err(format!("Could not connect to the server: {error}")),
            };
            callback_operation.set_password(None);
            // Timeout, cancellation and late callbacks cannot complete twice.
            if finish_reply(&reply, result) {
                timeout.remove();
            }
        },
    );
}

fn verified_mount_uri(request: &str, root: &str) -> NetworkResult<String> {
    let mut requested = url::Url::parse(request)
        .map_err(|_| NetworkError::new(NetworkErrorCode::InvalidUri, "Invalid server address"))?;
    let root = url::Url::parse(root).map_err(|_| {
        NetworkError::new(
            NetworkErrorCode::MountFailed,
            "Invalid server mount address",
        )
    })?;
    let default_port = |scheme: &str| match scheme {
        "sftp" => Some(22),
        "ftp" => Some(21),
        "ftps" => Some(990),
        "smb" => Some(445),
        "dav" => Some(80),
        "davs" => Some(443),
        _ => None,
    };
    let port = |url: &url::Url| url.port().or_else(|| default_port(url.scheme()));
    if requested.scheme() != root.scheme()
        || requested.host_str() != root.host_str()
        || port(&requested) != port(&root)
        || (!requested.username().is_empty()
            && !root.username().is_empty()
            && requested.username() != root.username())
    {
        return Err(NetworkError::new(
            NetworkErrorCode::MountFailed,
            "The connected mount does not match the requested server/account.",
        ));
    }
    if !root.username().is_empty() {
        requested.set_username(root.username()).map_err(|_| {
            NetworkError::new(NetworkErrorCode::InvalidUri, "Invalid server username")
        })?;
    }
    Ok(requested.to_string())
}

fn local_connection(
    uri: &str,
    username: Option<&str>,
    cancel: &gio::Cancellable,
) -> NetworkResult<(String, String)> {
    let effective_uri = saved::connection_uri(uri, username)?;
    let file = gio::File::for_uri(&effective_uri);
    // This asks GIO for the exact enclosing mount, never a mount picked merely
    // because it shares a protocol. It also recovers the username on keyring/key login.
    let mount = file.find_enclosing_mount(Some(cancel)).map_err(|error| {
        NetworkError::new(
            NetworkErrorCode::MountFailed,
            format!("Could not locate the connected server: {error}"),
        )
    })?;
    let root = mount.root();
    let effective_uri = verified_mount_uri(&effective_uri, root.uri().as_str())?;
    let file = gio::File::for_uri(&effective_uri);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if cancel.is_cancelled() {
            return Err(NetworkError::new(
                NetworkErrorCode::MountFailed,
                "Server mount lookup cancelled.",
            ));
        }
        if let Some(path) = file
            .path()
            .filter(|path| path.is_absolute() && path.is_dir())
        {
            return Ok((effective_uri, path.to_string_lossy().into_owned()));
        }
        if std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(NetworkError::new(NetworkErrorCode::MountFailed,
        "The server is mounted, but its local GVFS path is unavailable. Check that gvfs-fuse is installed and try again."))
}

pub(super) async fn mount(uri: String, app: tauri::AppHandle) -> NetworkResult<MountedNetwork> {
    // Validate before emitting events, presenting dialogs, or starting GIO.
    let uri = saved::connection_uri(&uri, None)?;
    tauri::async_runtime::spawn_blocking(super::gio_mounts::ensure_gvfsd_fuse_running).await?;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let callback_uri = uri.clone();
    let callback_app = app.clone();
    app.run_on_main_thread(move || mount_on_main_thread(callback_uri, callback_app, sender))?;
    let username = receiver
        .await
        .map_err(|_| {
            NetworkError::new(
                NetworkErrorCode::TaskFailed,
                "Server connection was interrupted.",
            )
        })?
        .map_err(|message| NetworkError::new(NetworkErrorCode::MountFailed, message))?;
    let lookup_cancel = gio::Cancellable::new();
    let worker_cancel = lookup_cancel.clone();
    let lookup = tauri::async_runtime::spawn_blocking(move || {
        local_connection(&uri, username.as_deref(), &worker_cancel)
    });
    let (uri, path) = match tokio::time::timeout(Duration::from_secs(8), lookup).await {
        Ok(result) => result??,
        Err(_) => {
            lookup_cancel.cancel();
            return Err(NetworkError::new(
                NetworkErrorCode::MountFailed,
                "The server connected, but its mount path lookup timed out. Try again.",
            ));
        }
    };
    tauri::async_runtime::spawn_blocking(move || {
        // Only a completed mount with an exact usable local path enters history.
        let warning = saved::remember(&uri).err().map(|_| {
            tracing::warn!("Server connected, but Browsey could not save the network address");
            "Connected, but the server address could not be saved. Check Browsey's data-directory permissions.".to_string()
        });
        Ok(MountedNetwork { uri, path, warning })
    }).await?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mount_guard_distinguishes_hosts_accounts_and_ports_not_child_paths() {
        assert_eq!(
            connection_key("sftp://alice@server/Photos"),
            connection_key("sftp://alice@server/Documents")
        );
        assert_ne!(
            connection_key("sftp://alice@server/"),
            connection_key("sftp://bob@server/")
        );
        assert_ne!(
            connection_key("sftp://server/"),
            connection_key("sftp://other/")
        );
        assert_ne!(
            connection_key("sftp://server/"),
            connection_key("sftp://server:2222/")
        );
    }

    #[test]
    fn cancelled_or_timed_out_request_cannot_be_completed_by_a_late_callback() {
        let (sender, mut receiver) = tokio::sync::oneshot::channel();
        let reply = Rc::new(RefCell::new(Some(sender)));
        assert!(finish_reply(&reply, Err("Connection cancelled.".into())));
        assert!(!finish_reply(&reply, Ok(Some("alice".into()))));
        assert_eq!(
            receiver.try_recv().unwrap(),
            Err("Connection cancelled.".into())
        );
    }

    #[test]
    fn exact_mount_check_rejects_another_host_account_protocol_or_port() {
        for root in [
            "sftp://other/",
            "sftp://bob@server/",
            "sftp://server:2222/",
            "ftp://server/",
        ] {
            assert!(
                verified_mount_uri("sftp://alice@server/Photos", root).is_err(),
                "{root}"
            );
        }
        assert_eq!(
            verified_mount_uri("sftp://server/Photos", "sftp://alice@server/").unwrap(),
            "sftp://alice@server/Photos"
        );
        assert_eq!(
            verified_mount_uri("sftp://server/Photos", "sftp://alice@server:22/").unwrap(),
            "sftp://alice@server/Photos"
        );
        assert_eq!(
            verified_mount_uri("sftp://server/Photos", "sftp://other%20user@server/").unwrap(),
            "sftp://other%20user@server/Photos"
        );
    }
}
