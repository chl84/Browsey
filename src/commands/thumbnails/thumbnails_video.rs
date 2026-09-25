use image::ImageReader;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use super::{
    error::{ThumbnailError, ThumbnailResult},
    thumb_log,
};

/// Render a video thumbnail by extracting a single frame using `ffmpeg`.
/// We rely on an available `ffmpeg` binary in PATH.
pub fn render_video_thumbnail(
    path: &Path,
    cache_path: &Path,
    max_dim: u32,
    ffmpeg_override: Option<&Path>,
    control: &super::control::Control,
) -> ThumbnailResult<(u32, u32)> {
    let ffmpeg = ffmpeg_override
        .and_then(|p| {
            if p.exists() {
                Some(p.to_path_buf())
            } else {
                None
            }
        })
        .or_else(which_ffmpeg)
        .ok_or_else(|| ThumbnailError::from_external_message("ffmpeg not found in PATH"))?;

    let tmp_path = cache_path.with_extension("tmp.png");
    let _cleanup = super::cache_flow::PendingFile(tmp_path.clone());

    // Seek to 1.5s to avoid black intro frames.
    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-y")
        .arg("-v")
        .arg("error")
        .arg("-hide_banner")
        .arg("-ss")
        .arg("1.5")
        .arg("-i")
        .arg(path)
        .arg("-frames:v")
        .arg("1")
        .arg("-vf")
        .arg(format!(
            "scale=min({0}\\,1280):-2:force_original_aspect_ratio=decrease",
            max_dim
        ))
        .arg(tmp_path.as_os_str());

    let status = run_with_timeout(cmd, Duration::from_secs(10), control)?;

    if !status.success() {
        return Err(ThumbnailError::from_external_message(format!(
            "ffmpeg failed with status {status}"
        )));
    }

    if tmp_path.exists() {
        std::fs::rename(&tmp_path, cache_path).map_err(|e| {
            ThumbnailError::from_external_message(format!("Move generated thumb failed: {e}"))
        })?;
    }

    let dims = ImageReader::open(cache_path)
        .map_err(|e| {
            ThumbnailError::from_external_message(format!("Read generated thumb failed: {e}"))
        })?
        .with_guessed_format()
        .map_err(|e| ThumbnailError::from_external_message(format!("Guess format failed: {e}")))?
        .into_dimensions()
        .map_err(|e| {
            ThumbnailError::from_external_message(format!("Read dimensions failed: {e}"))
        })?;

    thumb_log(&format!(
        "video thumbnail generated: source={} cache={} size={}x{}",
        path.display(),
        cache_path.display(),
        dims.0,
        dims.1
    ));

    Ok(dims)
}

fn which_ffmpeg() -> Option<PathBuf> {
    std::env::var("FFMPEG_BIN")
        .ok()
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .or_else(|| which::which("ffmpeg").ok())
}

fn run_with_timeout(
    mut cmd: Command,
    timeout: Duration,
    control: &super::control::Control,
) -> ThumbnailResult<std::process::ExitStatus> {
    let control =
        super::control::Control::new(control.cancelled.clone(), control.remaining(timeout)?);
    control.check()?;
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = cmd
        .spawn()
        .map_err(|e| ThumbnailError::from_external_message(format!("Spawn ffmpeg failed: {e}")))?;
    loop {
        if let Err(error) = control.check() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ThumbnailError::from_external_message(format!(
                    "Wait ffmpeg failed: {error}"
                )));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::render_video_thumbnail;
    use std::env;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, SystemTime};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn uniq_path(label: &str) -> std::path::PathBuf {
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!("browsey-thumb-video-test-{label}-{ts}"))
    }

    #[test]
    fn render_video_thumbnail_reports_missing_ffmpeg_cleanly() {
        let _guard = env_lock().lock().expect("env lock");
        let prev_ffmpeg_bin = env::var_os("FFMPEG_BIN");
        let prev_path = env::var_os("PATH");
        env::remove_var("FFMPEG_BIN");
        env::set_var("PATH", "");

        let source = uniq_path("source.mp4");
        let cache = uniq_path("cache.png");
        let control = crate::commands::thumbnails::control::Control::new(
            Default::default(),
            Duration::from_secs(1),
        );
        let err = render_video_thumbnail(&source, &cache, 96, None, &control)
            .expect_err("missing ffmpeg should fail");

        assert!(err.to_string().contains("ffmpeg not found in PATH"));

        match prev_ffmpeg_bin {
            Some(value) => env::set_var("FFMPEG_BIN", value),
            None => env::remove_var("FFMPEG_BIN"),
        }
        match prev_path {
            Some(value) => env::set_var("PATH", value),
            None => env::remove_var("PATH"),
        }
    }
}
