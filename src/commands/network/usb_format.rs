//! Long-running UDisks calls and device-scoped job progress.
//!
//! G_MAXINT disables GIO's default ~25s reply timeout. A transport failure is
//! NOT evidence that the server stopped writing: never retry an erase here.
use super::{
    error::{NetworkError, NetworkErrorCode, NetworkResult},
    mounts::UsbFormatProgress,
};
use gio::{
    glib::{variant::ObjectPath, variant::ToVariant, Variant},
    DBusCallFlags, DBusConnection,
};
use std::{
    collections::{HashMap, HashSet},
    sync::{mpsc, Mutex, MutexGuard},
    time::Duration,
};

const SERVICE: &str = "org.freedesktop.UDisks2";
const ROOT: &str = "/org/freedesktop/UDisks2";
const JOB: &str = "org.freedesktop.UDisks2.Job";
const PARTITION: &str = "org.freedesktop.UDisks2.Partition";
const BLOCK: &str = "org.freedesktop.UDisks2.Block";
const FORMAT_TIMEOUT: i32 = i32::MAX;
type Properties = HashMap<String, Variant>;
type Objects = HashMap<ObjectPath, HashMap<String, Properties>>;
type Reporter<'a> = &'a (dyn Fn(UsbFormatProgress) + Sync);

static FORMAT_LOCK: Mutex<()> = Mutex::new(());
pub(super) fn acquire_format_lock() -> NetworkResult<MutexGuard<'static, ()>> {
    FORMAT_LOCK.try_lock().map_err(|_| {
        NetworkError::new(
            NetworkErrorCode::FormatBusy,
            "Another USB formatting operation is still running. Wait for it to finish.",
        )
    })
}

pub(super) struct Client {
    connection: DBusConnection,
    disk: String,
    destination: String,
}

#[derive(Debug)]
struct JobProgress {
    operation: String,
    percent: Option<f64>,
}

fn jobs_for_disk(objects: &Objects, disk: &str) -> Vec<JobProgress> {
    let mut targets = HashSet::from([disk.to_owned()]);
    // Exact object relationships, not a prefix match (sda != sdaa).
    for (path, interfaces) in objects {
        if path.as_str() == disk {
            if let Some(drive) = interfaces
                .get(BLOCK)
                .and_then(|p| p.get("Drive"))
                .and_then(|v| v.get::<ObjectPath>())
            {
                if drive.as_str() != "/" {
                    targets.insert(drive.to_string());
                }
            }
        }
        if interfaces
            .get(PARTITION)
            .and_then(|p| p.get("Table"))
            .and_then(|v| v.get::<ObjectPath>())
            .is_some_and(|table| table.as_str() == disk)
        {
            targets.insert(path.to_string());
        }
    }
    objects
        .values()
        .filter_map(|interfaces| {
            let props = interfaces.get(JOB)?;
            let affected = props.get("Objects")?.get::<Vec<ObjectPath>>()?;
            if !affected.iter().any(|path| targets.contains(path.as_str())) {
                return None;
            }
            let valid = props
                .get("ProgressValid")
                .and_then(|v| v.get::<bool>())
                .unwrap_or(false);
            let percent = props
                .get("Progress")
                .and_then(|v| v.get::<f64>())
                .filter(|value| valid && value.is_finite() && (0.0..=1.0).contains(value))
                .map(|value| value * 100.0);
            Some(JobProgress {
                operation: props
                    .get("Operation")
                    .and_then(|v| v.get::<String>())
                    .unwrap_or_default(),
                percent,
            })
        })
        .collect()
}

fn progress_for_jobs(jobs: &[JobProgress], fallback: &str) -> UsbFormatProgress {
    let Some(job) = jobs.first().filter(|_| jobs.len() == 1) else {
        return UsbFormatProgress::new(fallback, None);
    };
    let phase = match job.operation.as_str() {
        "format-erase" => "Erasing filesystem signatures",
        "format-mkfs" => "Creating filesystem",
        "partition-create" => "Creating partition",
        "filesystem-mount" => "Mounting USB drive",
        _ => fallback,
    };
    UsbFormatProgress::new(phase, job.percent)
}

fn operation_error(message: &str) -> NetworkError {
    let definitive = message.starts_with("GDBus.Error:org.freedesktop.UDisks2.Error.")
        || message.starts_with("GDBus.Error:org.freedesktop.DBus.Error.AccessDenied:");
    if definitive {
        NetworkError::new(NetworkErrorCode::FormatFailed, message)
    } else {
        NetworkError::new(NetworkErrorCode::FormatStatusUnknown, format!(
            "Could not confirm the formatting result. The system may still be working or the drive may already be formatted. Do not unplug or format it again until its status has been checked. Details: {message}"))
    }
}

impl Client {
    pub(super) fn connect(disk: &str) -> NetworkResult<Self> {
        let connection =
            gio::bus_get_sync(gio::BusType::System, gio::Cancellable::NONE).map_err(|error| {
                NetworkError::new(
                    NetworkErrorCode::FormatNotAllowed,
                    format!("Could not connect to UDisks: {error}"),
                )
            })?;
        Ok(Self {
            connection,
            disk: disk.to_owned(),
            destination: SERVICE.into(),
        })
    }

    fn jobs(&self) -> NetworkResult<Vec<JobProgress>> {
        let reply = self
            .connection
            .call_sync(
                Some(&self.destination),
                ROOT,
                "org.freedesktop.DBus.ObjectManager",
                "GetManagedObjects",
                None,
                None,
                DBusCallFlags::NONE,
                2000,
                gio::Cancellable::NONE,
            )
            .map_err(|error| operation_error(&error.to_string()))?;
        let (objects,) = reply
            .get::<(Objects,)>()
            .ok_or_else(|| operation_error("Invalid UDisks job snapshot"))?;
        Ok(jobs_for_disk(&objects, &self.disk))
    }

    pub(super) fn ensure_idle(&self) -> NetworkResult<()> {
        if !self.jobs()?.is_empty() {
            return Err(NetworkError::new(NetworkErrorCode::FormatBusy,
                "The system is still working on this USB drive. Wait for the current operation to finish before formatting."));
        }
        Ok(())
    }

    fn call(
        &self,
        interface: &str,
        method: &str,
        parameters: &Variant,
        phase: &str,
        report: Reporter<'_>,
    ) -> NetworkResult<Variant> {
        report(UsbFormatProgress::new(phase, None));
        std::thread::scope(|scope| {
            let (stop, stopped) = mpsc::channel();
            let monitor = std::thread::Builder::new()
                .name("usb-format-progress".into())
                .spawn_scoped(scope, move || {
                    loop {
                        match self.jobs() {
                            Ok(jobs) => report(progress_for_jobs(&jobs, phase)),
                            // The method reply remains authoritative. Missing progress
                            // must not turn an ongoing format into a failure.
                            Err(_) => report(UsbFormatProgress::new(phase, None)),
                        }
                        if !matches!(
                            stopped.recv_timeout(Duration::from_millis(500)),
                            Err(mpsc::RecvTimeoutError::Timeout)
                        ) {
                            break;
                        }
                    }
                })
                .map_err(|error| {
                    NetworkError::new(NetworkErrorCode::TaskFailed, error.to_string())
                })?;
            let result = self.connection.call_sync(
                Some(&self.destination),
                &self.disk,
                interface,
                method,
                Some(parameters),
                None,
                DBusCallFlags::ALLOW_INTERACTIVE_AUTHORIZATION,
                FORMAT_TIMEOUT,
                gio::Cancellable::NONE,
            );
            let _ = stop.send(());
            let _ = monitor.join();
            match result {
                Ok(reply) => Ok(reply),
                Err(error) => {
                    // If the connection still works, do not report a terminal
                    // result while a matching server-side job is active.
                    while let Ok(jobs) = self.jobs() {
                        if jobs.is_empty() {
                            break;
                        }
                        report(progress_for_jobs(&jobs, "Waiting for the system to finish"));
                        std::thread::sleep(Duration::from_millis(500));
                    }
                    Err(operation_error(&error.to_string()))
                }
            }
        })
    }

    pub(super) fn format_disk(
        &self,
        filesystem: &str,
        label: Option<&str>,
        report: Reporter<'_>,
    ) -> NetworkResult<String> {
        let empty = Properties::new();
        self.call(
            BLOCK,
            "Format",
            &("gpt", &empty).to_variant(),
            "Creating partition table",
            report,
        )?;
        let mut options = Properties::new();
        if let Some(label) = label {
            options.insert("label".into(), label.to_variant());
        }
        let result = self.call(
            "org.freedesktop.UDisks2.PartitionTable",
            "CreatePartitionAndFormat",
            &(0u64, 0u64, "", "", &empty, filesystem, options).to_variant(),
            "Creating partition and filesystem",
            report,
        )?;
        let (partition,) = result
            .get::<(ObjectPath,)>()
            .ok_or_else(|| operation_error("UDisks returned no new partition"))?;
        // Read the authoritative device name instead of decoding a D-Bus path.
        let reply = self
            .connection
            .call_sync(
                Some(&self.destination),
                partition.as_str(),
                "org.freedesktop.DBus.Properties",
                "Get",
                Some(&(BLOCK, "Device").to_variant()),
                None,
                DBusCallFlags::NONE,
                2000,
                gio::Cancellable::NONE,
            )
            .map_err(|error| operation_error(&error.to_string()))?;
        let (value,) = reply
            .get::<(Variant,)>()
            .ok_or_else(|| operation_error("Invalid partition device reply"))?;
        let mut bytes = value
            .get::<Vec<u8>>()
            .ok_or_else(|| operation_error("Invalid partition device name"))?;
        if bytes.last() == Some(&0) {
            bytes.pop();
        }
        let device = String::from_utf8(bytes)
            .map_err(|_| operation_error("Invalid partition device encoding"))?;
        if !device.starts_with("/dev/") {
            return Err(operation_error("Invalid partition device path"));
        }
        Ok(device)
    }
}

#[cfg(test)]
#[path = "usb_format_test_bus.rs"]
mod test_bus;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::domain::DomainError;

    fn path(value: &str) -> ObjectPath {
        ObjectPath::try_from(value).unwrap()
    }

    #[test]
    fn tracks_only_jobs_for_the_selected_disk_and_its_partitions() {
        let disk = "/org/freedesktop/UDisks2/block_devices/sda";
        let partition = format!("{disk}1");
        let mut objects = Objects::new();
        objects.insert(
            path(&partition),
            HashMap::from([(
                PARTITION.into(),
                HashMap::from([("Table".into(), path(disk).to_variant())]),
            )]),
        );
        let job = |target: &str, valid: bool| {
            HashMap::from([(
                JOB.into(),
                HashMap::from([
                    ("Objects".into(), vec![path(target)].to_variant()),
                    ("ProgressValid".into(), valid.to_variant()),
                    ("Progress".into(), 0.42f64.to_variant()),
                    ("Operation".into(), "format-mkfs".to_variant()),
                ]),
            )])
        };
        objects.insert(path("/jobs/1"), job(&partition, true));
        objects.insert(path("/jobs/2"), job(&format!("{disk}a1"), true));
        // Exercise the exact GVariant shape returned by GetManagedObjects.
        let (decoded,) = (objects,).to_variant().get::<(Objects,)>().unwrap();
        let jobs = jobs_for_disk(&decoded, disk);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].percent, Some(42.0));
        assert_eq!(
            progress_for_jobs(&jobs, "Waiting").phase,
            "Creating filesystem"
        );
    }

    #[test]
    fn unknown_or_multiple_job_progress_is_indeterminate() {
        assert!(progress_for_jobs(&[], "Waiting").percent.is_none());
        let jobs = vec![JobProgress {
            operation: "format-mkfs".into(),
            percent: None,
        }];
        assert!(progress_for_jobs(&jobs, "Waiting").percent.is_none());
        let jobs = vec![
            JobProgress {
                operation: "a".into(),
                percent: Some(90.0),
            },
            JobProgress {
                operation: "b".into(),
                percent: Some(20.0),
            },
        ];
        assert!(progress_for_jobs(&jobs, "Waiting").percent.is_none());
    }

    #[test]
    fn invalid_job_percentages_are_never_displayed() {
        let disk = "/org/freedesktop/UDisks2/block_devices/sda";
        for (valid, value) in [(false, 0.5), (true, f64::NAN), (true, -0.1), (true, 1.1)] {
            let objects = Objects::from([(
                path("/jobs/1"),
                HashMap::from([(
                    JOB.into(),
                    Properties::from([
                        ("Objects".into(), vec![path(disk)].to_variant()),
                        ("ProgressValid".into(), valid.to_variant()),
                        ("Progress".into(), value.to_variant()),
                    ]),
                )]),
            )]);
            let jobs = jobs_for_disk(&objects, disk);
            assert_eq!(jobs.len(), 1);
            assert!(jobs[0].percent.is_none());
        }
    }

    #[test]
    fn lost_replies_are_unknown_not_format_failed() {
        assert_eq!(
            operation_error("Timeout was reached").code_str(),
            "format_status_unknown"
        );
        assert_eq!(
            operation_error("The connection is closed").code_str(),
            "format_status_unknown"
        );
        assert_eq!(
            operation_error("GDBus.Error:org.freedesktop.UDisks2.Error.Failed: mkfs failed")
                .code_str(),
            "format_failed"
        );
        assert_eq!(FORMAT_TIMEOUT, i32::MAX); // GIO's no-timeout sentinel, not -1.
    }

    #[test]
    fn duplicate_format_is_rejected_until_the_first_guard_is_released() {
        let guard = acquire_format_lock().unwrap();
        assert!(acquire_format_lock().is_err());
        drop(guard);
        assert!(acquire_format_lock().is_ok());
    }
}
