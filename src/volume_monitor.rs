//! UDisks notifications supplement the frontend's periodic mount refresh.
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::Mutex,
};

#[derive(Default)]
pub struct VolumeMonitor(Mutex<Option<Child>>);

fn is_volume_signal(line: &str) -> bool {
    line.starts_with("/org/freedesktop/UDisks2")
        && [
            ".InterfacesAdded",
            ".InterfacesRemoved",
            ".PropertiesChanged",
        ]
        .iter()
        .any(|signal| line.contains(signal))
}

impl VolumeMonitor {
    pub fn start(&self, app: tauri::AppHandle) -> std::io::Result<()> {
        let mut slot = self
            .0
            .lock()
            .map_err(|_| std::io::Error::other("volume monitor lock poisoned"))?;
        if slot.is_some() {
            return Ok(());
        }
        let mut child = Command::new("gdbus")
            .args(["monitor", "--system", "--dest", "org.freedesktop.UDisks2"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdout = child.stdout.take().expect("piped monitor stdout");
        if let Err(error) = std::thread::Builder::new()
            .name("volume-monitor".into())
            .spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    if crate::runtime_lifecycle::is_shutting_down(&app) {
                        break;
                    }
                    if is_volume_signal(&line) {
                        crate::runtime_lifecycle::emit_if_running(&app, "volumes-changed", ());
                    }
                }
            })
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        *slot = Some(child);
        Ok(())
    }

    pub fn stop(&self) {
        if let Ok(mut slot) = self.0.lock() {
            if let Some(mut child) = slot.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

impl Drop for VolumeMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_udisks_object_and_property_changes_trigger_refresh() {
        assert!(is_volume_signal(
            "/org/freedesktop/UDisks2: org.freedesktop.DBus.ObjectManager.InterfacesAdded (...)"
        ));
        assert!(is_volume_signal(
            "/org/freedesktop/UDisks2: org.freedesktop.DBus.ObjectManager.InterfacesRemoved (...)"
        ));
        assert!(is_volume_signal("/org/freedesktop/UDisks2/block_devices/sda1: org.freedesktop.DBus.Properties.PropertiesChanged (...)"));
        assert!(!is_volume_signal(
            "Monitoring signals on object /org/freedesktop/UDisks2"
        ));
        assert!(!is_volume_signal(
            "/other: org.freedesktop.DBus.Properties.PropertiesChanged (...)"
        ));
    }
}
