//! Opt-in end-to-end D-Bus contract test. Never connects to the system bus.
use super::*;
use crate::errors::domain::DomainError;
use gio::glib;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

#[test]
#[ignore = "run inside dbus-run-session with BROWSEY_PRIVATE_TEST_BUS=1; takes 26 seconds"]
fn waits_beyond_old_timeout_and_reports_real_job_progress() {
    assert_eq!(
        std::env::var("BROWSEY_PRIVATE_TEST_BUS").as_deref(),
        Ok("1")
    );
    let (ready_tx, ready_rx) = mpsc::channel();
    let disk = "/org/freedesktop/UDisks2/block_devices/test_disk";
    let partition = "/org/freedesktop/UDisks2/block_devices/test_disk1";
    let active = Arc::new(AtomicBool::new(false));
    let destructive_calls = Arc::new(AtomicUsize::new(0));
    let missing_ntfs = Arc::new(AtomicBool::new(false));
    let server_calls = destructive_calls.clone();
    let server_missing = missing_ntfs.clone();
    let server = std::thread::spawn(move || {
        let context = glib::MainContext::default();
        context.with_thread_default(|| {
            let connection = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).unwrap();
            let loop_ = glib::MainLoop::new(Some(&context), false);
            let info = gio::DBusNodeInfo::for_xml(r#"<node>
              <interface name="org.freedesktop.UDisks2.Manager"><method name="CanFormat"><arg direction="in" type="s"/><arg direction="out" type="(bs)"/></method></interface>
              <interface name="org.freedesktop.DBus.ObjectManager"><method name="GetManagedObjects"><arg direction="out" type="a{oa{sa{sv}}}"/></method></interface>
              <interface name="org.freedesktop.UDisks2.Block"><method name="Format"><arg direction="in" type="s"/><arg direction="in" type="a{sv}"/></method><property name="Device" type="ay" access="read"/></interface>
              <interface name="org.freedesktop.UDisks2.PartitionTable"><method name="CreatePartitionAndFormat">
                <arg direction="in" type="t"/><arg direction="in" type="t"/><arg direction="in" type="s"/><arg direction="in" type="s"/>
                <arg direction="in" type="a{sv}"/><arg direction="in" type="s"/><arg direction="in" type="a{sv}"/><arg direction="out" type="o"/>
              </method></interface>
            </node>"#).unwrap();
            let mut registrations = Vec::new();
            for (object, interface) in [("/org/freedesktop/UDisks2/Manager", "org.freedesktop.UDisks2.Manager"), (ROOT, "org.freedesktop.DBus.ObjectManager"), (disk, BLOCK),
                (disk, "org.freedesktop.UDisks2.PartitionTable"), (partition, BLOCK)] {
                let active = active.clone();
                let destructive_calls = server_calls.clone();
                let missing_ntfs = server_missing.clone();
                registrations.push(connection.register_object(object, &info.lookup_interface(interface).unwrap(),
                    move |_, _, _, _, method, params, invocation| match method {
                        "CanFormat" => {
                            let (fs,) = params.get::<(String,)>().unwrap();
                            if !["ext4", "btrfs", "exfat", "vfat", "ntfs"].contains(&fs.as_str()) {
                                invocation.return_dbus_error("org.freedesktop.UDisks2.Error.NotSupported", "Unsupported filesystem");
                            } else {
                                let missing = fs == "ntfs" && missing_ntfs.load(Ordering::SeqCst);
                                invocation.return_value(Some(&((!missing, if missing { "mkntfs" } else { "" }),).to_variant()));
                            }
                        }
                        "GetManagedObjects" => {
                            let mut objects = Objects::new();
                            if active.load(Ordering::SeqCst) {
                                objects.insert(ObjectPath::try_from(partition).unwrap(), HashMap::from([
                                    (PARTITION.into(), HashMap::from([("Table".into(), ObjectPath::try_from(disk).unwrap().to_variant())]))]));
                                objects.insert(ObjectPath::try_from("/org/freedesktop/UDisks2/jobs/1").unwrap(), HashMap::from([
                                    (JOB.into(), HashMap::from([
                                        ("Objects".into(), vec![ObjectPath::try_from(partition).unwrap()].to_variant()),
                                        ("ProgressValid".into(), true.to_variant()), ("Progress".into(), 0.42f64.to_variant()),
                                        ("Operation".into(), "format-mkfs".to_variant()),
                                    ]))]));
                            }
                            invocation.return_value(Some(&(objects,).to_variant()));
                        }
                        "Format" => {
                            destructive_calls.fetch_add(1, Ordering::SeqCst);
                            assert_eq!(params.get::<(String, Properties)>().unwrap().0, "gpt");
                            invocation.return_value(Some(&().to_variant()));
                        }
                        "CreatePartitionAndFormat" => {
                            destructive_calls.fetch_add(1, Ordering::SeqCst);
                            let (_, _, _, _, _, fs, options) = params.get::<(u64, u64, String, String, Properties, String, Properties)>().unwrap();
                            assert!(["ext4", "btrfs", "exfat", "vfat", "ntfs"].contains(&fs.as_str()));
                            assert_eq!(options["take-ownership"].get::<bool>(), Some(true));
                            assert_eq!(options["update-partition-type"].get::<bool>(), Some(true));
                            if fs != "ext4" {
                                if fs == "ntfs" && options.contains_key("label") {
                                    assert_eq!(options["label"].get::<String>().as_deref(), Some("Windows_backup_2026"));
                                } else {
                                    assert!(!options.contains_key("label"));
                                }
                                invocation.return_value(Some(&(ObjectPath::try_from(partition).unwrap(),).to_variant()));
                                return;
                            }
                            if options["label"].get::<String>().as_deref() == Some("FAIL") {
                                invocation.return_dbus_error("org.freedesktop.UDisks2.Error.Failed", "Simulated mkfs failure");
                                return;
                            }
                            assert_eq!(options["label"].get::<String>().as_deref(), Some("TEST"));
                            active.store(true, Ordering::SeqCst);
                            let active = active.clone();
                            glib::timeout_add_local_once(Duration::from_secs(26), move || {
                                active.store(false, Ordering::SeqCst);
                                invocation.return_value(Some(&(ObjectPath::try_from(partition).unwrap(),).to_variant()));
                            });
                        }
                        _ => invocation.return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", "Unexpected test method"),
                    },
                    |_, _, _, _, _| b"/dev/browsey-test-only\0".to_vec().to_variant(),
                    |_, _, _, _, _, _| false).unwrap());
            }
            ready_tx.send((connection.clone(), loop_.clone())).unwrap();
            loop_.run();
            for registration in registrations { connection.unregister_object(registration).unwrap(); }
        }).unwrap();
    });
    let (connection, loop_) = ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let client = Client {
        disk: disk.into(),
        destination: connection.unique_name().unwrap().to_string(),
        connection,
    };
    missing_ntfs.store(true, Ordering::SeqCst);
    let unavailable = client.format_disk("ntfs", None, &|_| {}).unwrap_err();
    assert_eq!(unavailable.code_str(), "format_not_allowed");
    assert!(unavailable.to_string().contains("mkntfs"));
    assert_eq!(destructive_calls.load(Ordering::SeqCst), 0);
    let unsupported = client
        .format_disk("unsupported", None, &|_| {})
        .unwrap_err();
    assert_eq!(unsupported.code_str(), "format_not_allowed");
    assert_eq!(destructive_calls.load(Ordering::SeqCst), 0);
    missing_ntfs.store(false, Ordering::SeqCst);
    client.ensure_can_format("ntfs").unwrap();
    let observed = AtomicBool::new(false);
    let busy_rejected = AtomicBool::new(false);
    let started = std::time::Instant::now();
    let result = client.format_disk("ext4", Some("TEST"), &|progress| {
        if progress.percent == Some(42.0) {
            observed.store(true, Ordering::SeqCst);
            busy_rejected.store(client.ensure_idle().is_err(), Ordering::SeqCst);
        }
    });
    let failed = client.format_disk("ext4", Some("FAIL"), &|_| {});
    let other_filesystems: Vec<_> = ["btrfs", "exfat", "vfat"]
        .iter()
        .map(|filesystem| client.format_disk(filesystem, None, &|_| {}))
        .collect();
    let ntfs = client.format_disk("ntfs", Some("Windows_backup_2026"), &|_| {});
    let ntfs_without_label = client.format_disk("ntfs", None, &|_| {});
    loop_.quit();
    server.join().unwrap();
    assert_eq!(result.unwrap(), "/dev/browsey-test-only");
    assert!(started.elapsed() >= Duration::from_secs(26));
    assert!(observed.load(Ordering::SeqCst));
    assert!(busy_rejected.load(Ordering::SeqCst));
    assert_eq!(failed.unwrap_err().code_str(), "format_failed");
    assert_eq!(ntfs.unwrap(), "/dev/browsey-test-only");
    assert_eq!(ntfs_without_label.unwrap(), "/dev/browsey-test-only");
    for result in other_filesystems {
        assert_eq!(result.unwrap(), "/dev/browsey-test-only");
    }
}
