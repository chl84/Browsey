//! A portal token belongs to one drag, not to its first reader.
use std::{collections::HashMap, fs::OpenOptions, os::unix::fs::OpenOptionsExt};

use gio::{glib, prelude::*};
use glib::variant::{Handle, ToVariant};
use url::Url;

const SERVICE: &str = "org.freedesktop.portal.Documents";
const PATH: &str = "/org/freedesktop/portal/documents";
const INTERFACE: &str = "org.freedesktop.portal.FileTransfer";
const TIMEOUT_MS: i32 = 2000;

fn options() -> HashMap<String, glib::Variant> {
    HashMap::new()
}

pub(super) fn is_target(name: &str) -> bool {
    matches!(
        name,
        "application/vnd.portal.filetransfer" | "application/vnd.portal.files"
    )
}

pub(super) struct Transfer {
    connection: gio::DBusConnection,
    key: String,
}

impl Transfer {
    pub(super) fn register(uris: &[String]) -> Result<Self, String> {
        let connection = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE)
            .map_err(|err| err.to_string())?;
        let mut settings = options();
        // GTK4 may retrieve once during motion and again at drop. GTK3's
        // set_uris helper uses the default autostop=true, invalidating that key.
        settings.insert("autostop".into(), false.to_variant());
        let reply = connection
            .call_sync(
                Some(SERVICE),
                PATH,
                INTERFACE,
                "StartTransfer",
                Some(&(settings,).to_variant()),
                None,
                gio::DBusCallFlags::NONE,
                TIMEOUT_MS,
                gio::Cancellable::NONE,
            )
            .map_err(|err| err.to_string())?;
        let (key,) = reply
            .get::<(String,)>()
            .ok_or("Invalid portal transfer response")?;
        let transfer = Self { connection, key };
        // Drop stops even partially registered transfers on any error below.
        for chunk in uris.chunks(16) {
            let fds = gio::UnixFDList::new();
            let mut handles = Vec::with_capacity(chunk.len());
            for uri in chunk {
                let path = Url::parse(uri)
                    .ok()
                    .and_then(|url| url.to_file_path().ok())
                    .ok_or("Invalid local file URI")?;
                // O_PATH supports directories and does not read file contents.
                let file = OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_PATH | libc::O_CLOEXEC)
                    .open(path)
                    .map_err(|err| err.to_string())?;
                handles.push(Handle(fds.append(file).map_err(|err| err.to_string())?));
            }
            // gio 0.18's call_with_unix_fd_list_sync assumes a non-null FD
            // list in the reply; AddFiles legitimately returns none. Use the
            // message API so that an empty reply cannot panic in GTK's callback.
            let message =
                gio::DBusMessage::new_method_call(Some(SERVICE), PATH, Some(INTERFACE), "AddFiles");
            message.set_body(&(&transfer.key, handles, options()).to_variant());
            message.set_unix_fd_list(Some(&fds));
            let (reply, _) = transfer
                .connection
                .send_message_with_reply_sync(
                    &message,
                    gio::DBusSendMessageFlags::NONE,
                    TIMEOUT_MS,
                    gio::Cancellable::NONE,
                )
                .map_err(|err| err.to_string())?;
            if reply.message_type() != gio::DBusMessageType::MethodReturn {
                let detail = reply
                    .body()
                    .and_then(|body| body.get::<(String,)>())
                    .map(|(text,)| text)
                    .unwrap_or_default();
                return Err(format!(
                    "{}: {detail}",
                    reply
                        .error_name()
                        .as_deref()
                        .unwrap_or("Unexpected portal reply")
                ));
            }
        }
        Ok(transfer)
    }

    pub(super) fn key(&self) -> &str {
        &self.key
    }
}

impl Drop for Transfer {
    fn drop(&mut self) {
        // Queue cleanup without blocking GTK teardown, running a nested main
        // loop, or capturing any Tauri window/channel in a completion callback.
        let message =
            gio::DBusMessage::new_method_call(Some(SERVICE), PATH, Some(INTERFACE), "StopTransfer");
        message.set_body(&(&self.key,).to_variant());
        message.set_flags(gio::DBusMessageFlags::NO_REPLY_EXPECTED);
        if let Err(err) = self
            .connection
            .send_message(&message, gio::DBusSendMessageFlags::NONE)
        {
            tracing::warn!("Could not close native drag portal transfer: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_portal_targets_use_tokens() {
        assert!(is_target("application/vnd.portal.filetransfer"));
        assert!(is_target("application/vnd.portal.files"));
        assert!(!is_target("text/uri-list"));
        assert!(!is_target("text/plain"));
    }

    #[test]
    #[ignore = "requires a running FileTransfer portal on the session bus"]
    fn portal_token_survives_multiple_reads_and_expires_on_drop() {
        let root = std::env::temp_dir().join(format!(
            "browsey-portal-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        // Cross the 16-FD batch boundary, include a directory and odd filenames.
        let paths = (0..18)
            .map(|i| {
                let path = root.join(format!("æ # {i}"));
                if i == 0 {
                    std::fs::create_dir(&path).unwrap();
                } else {
                    std::fs::write(&path, b"disposable portal fixture").unwrap();
                }
                path
            })
            .collect::<Vec<_>>();
        let uris = paths
            .iter()
            .map(|p| Url::from_file_path(p).unwrap().into())
            .collect::<Vec<_>>();
        let transfer = Transfer::register(&uris).unwrap();
        let connection = transfer.connection.clone();
        let key = transfer.key.clone();
        let retrieve = || {
            connection.call_sync(
                Some(SERVICE),
                PATH,
                INTERFACE,
                "RetrieveFiles",
                Some(&(&key, options()).to_variant()),
                None,
                gio::DBusCallFlags::NONE,
                TIMEOUT_MS,
                gio::Cancellable::NONE,
            )
        };
        for _ in 0..3 {
            let (files,) = retrieve().unwrap().get::<(Vec<String>,)>().unwrap();
            let actual = files
                .into_iter()
                .map(std::path::PathBuf::from)
                .collect::<Vec<_>>();
            assert_eq!(actual, paths);
        }
        drop(transfer);
        // Calls on the same connection are ordered after queued StopTransfer.
        let err = retrieve().unwrap_err();
        assert!(err.to_string().contains("Invalid transfer"), "{err}");
        let missing = Url::from_file_path(root.join("missing")).unwrap().into();
        assert!(Transfer::register(&[missing]).is_err());
    }
}
