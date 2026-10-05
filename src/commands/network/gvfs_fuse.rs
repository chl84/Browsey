//! Recover the session's POSIX bridge without remounting its GIO backends.
use crate::binary_resolver::resolve_binary_with_overrides_checked;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};

const LIBEXEC_PATHS: &[&str] = &[
    "/usr/lib/gvfsd-fuse",
    "/usr/libexec/gvfsd-fuse",
    "/usr/lib/gvfs/gvfsd-fuse",
    "/usr/libexec/gvfs/gvfsd-fuse",
];
const RETRY_AFTER: Duration = Duration::from_secs(10);
static LAST_ATTEMPT: Mutex<Option<Instant>> = Mutex::new(None);

pub(super) fn root() -> Option<PathBuf> {
    dirs_next::runtime_dir().map(|path| path.join("gvfs"))
}

fn mountinfo_has_bridge(contents: &str, root: &Path) -> bool {
    // Mountinfo escapes these four characters, including a literal backslash.
    let escaped = root
        .to_string_lossy()
        .replace('\\', "\\134")
        .replace(' ', "\\040")
        .replace('\t', "\\011")
        .replace('\n', "\\012");
    contents.lines().any(|line| {
        let Some((fields, filesystem)) = line.split_once(" - ") else {
            return false;
        };
        fields.split_whitespace().nth(4) == Some(escaped.as_str())
            && filesystem.split_whitespace().next() == Some("fuse.gvfsd-fuse")
    })
}

fn bridge_mounted(root: &Path) -> bool {
    // A process name (possibly another user's) or an empty directory is not
    // evidence that this session actually has the required FUSE mount.
    fs::read_to_string("/proc/self/mountinfo")
        .is_ok_and(|contents| mountinfo_has_bridge(&contents, root))
}

fn retry_due(last: Option<Instant>, now: Instant) -> bool {
    last.is_none_or(|last| now.saturating_duration_since(last) >= RETRY_AFTER)
}

pub(super) fn ensure_running() -> bool {
    let Some(root) = root() else { return false };
    let mut last = LAST_ATTEMPT
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if bridge_mounted(&root) {
        // Never cache success indefinitely: the bridge can disappear while
        // Browsey remains open. A later loss should trigger immediate recovery.
        *last = None;
        return true;
    }
    if !retry_due(*last, Instant::now()) {
        return false;
    }
    *last = Some(Instant::now());
    let program = match resolve_binary_with_overrides_checked(
        "gvfsd-fuse",
        LIBEXEC_PATHS.iter().map(PathBuf::from),
    ) {
        Ok(program) => program,
        Err(_) => {
            tracing::warn!("GVFS FUSE bridge executable is unavailable; install gvfs-fuse");
            return false;
        }
    };
    if fs::create_dir_all(&root).is_err() {
        tracing::warn!("Could not prepare the session's GVFS FUSE mount point");
        return false;
    }
    let mut child = match Command::new(program)
        .arg(&root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => {
            tracing::warn!("Could not start the GVFS FUSE bridge");
            return false;
        }
    };
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut reaped = false;
    let ready = loop {
        if bridge_mounted(&root) {
            break true;
        }
        if !reaped {
            match child.try_wait() {
                Ok(Some(status)) => {
                    reaped = true;
                    if !status.success() {
                        break false;
                    }
                }
                Ok(None) => {}
                Err(_) => break false,
            }
        }
        if Instant::now() >= deadline {
            break false;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    // Reap our launcher, but never kill a bridge shared by desktop applications.
    if !reaped {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
    if ready {
        *last = None;
    } else {
        tracing::warn!("GVFS FUSE bridge did not expose the session mount point");
    }
    ready
}

/// Recover only explicit paths inside the current session's GVFS mount point.
/// Ordinary missing local paths must not start daemons or acquire a retry delay.
pub(crate) fn recover_path(path: &Path) -> bool {
    root().is_some_and(|root| path.starts_with(root)) && ensure_running()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readiness_requires_the_exact_sessions_gvfs_filesystem() {
        let contents = "41 30 0:40 / /run/user/2000/gvfs rw - fuse.gvfsd-fuse gvfsd-fuse rw\n\
            42 30 0:41 / /run/user/1000/gvfs-other rw - fuse.gvfsd-fuse gvfsd-fuse rw\n\
            43 30 0:42 / /run/user/1000/gvfs rw - tmpfs tmpfs rw\n";
        let root = Path::new("/run/user/1000/gvfs");
        assert!(!mountinfo_has_bridge(contents, root));
        let live =
            "44 30 0:43 / /run/user/1000/gvfs rw,nosuid shared:1 - fuse.gvfsd-fuse gvfsd-fuse rw\n";
        assert!(mountinfo_has_bridge(live, root));
        // Re-reading an empty mount table must not retain a previous success.
        assert!(!mountinfo_has_bridge("", root));
    }

    #[test]
    fn mountinfo_handles_escaped_runtime_paths_and_malformed_lines() {
        let contents = "bad line\n44 30 0:43 / /runtime\\040dir\\134test/gvfs rw - fuse.gvfsd-fuse gvfsd-fuse rw\n";
        assert!(mountinfo_has_bridge(
            contents,
            Path::new("/runtime dir\\test/gvfs")
        ));
        assert!(!mountinfo_has_bridge(
            contents,
            Path::new("/runtime dir/test/gvfs")
        ));
    }

    #[test]
    fn only_failed_startups_are_throttled() {
        let now = Instant::now();
        assert!(retry_due(None, now));
        assert!(!retry_due(Some(now), now + Duration::from_secs(9)));
        assert!(retry_due(Some(now), now + RETRY_AFTER));
    }

    // Explicit, metadata-only smoke test; never discover mounts or list folders.
    #[test]
    #[ignore = "requires an explicitly approved existing GVFS ai_agent_testfolder"]
    fn real_bridge_exposes_approved_directory() {
        let path =
            PathBuf::from(std::env::var("BROWSEY_TEST_GVFS_DIR").expect("explicit test path"));
        assert_eq!(path.file_name().unwrap(), "ai_agent_testfolder");
        assert!(path.starts_with(root().expect("session runtime")));
        assert!(!path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir)));
        assert!(recover_path(&path), "session FUSE bridge unavailable");
        assert!(
            crate::fs_utils::sanitize_path_follow(path.to_str().unwrap(), false)
                .unwrap()
                .is_dir(),
            "approved directory unavailable"
        );
    }
}
