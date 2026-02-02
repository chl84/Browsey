use image::ImageReader;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use super::thumb_log;

/// Render a video thumbnail by extracting a single frame using `ffmpeg`.
/// We rely on an available `ffmpeg` binary in PATH.
pub fn render_video_thumbnail(
    path: &Path,
    cache_path: &Path,
    max_dim: u32,
    generation: Option<&str>,
    ffmpeg_override: Option<&Path>,
) -> Result<(u32, u32), String> {
    let ffmpeg = ffmpeg_override
        .and_then(|p| {
            if p.exists() {
                Some(p.to_path_buf())
            } else {
                None
            }
        })
        .or_else(which_ffmpeg)
        .ok_or("ffmpeg not found in PATH")?;

    let tmp_path = cache_path.with_extension("tmp.png");

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

    let status = run_with_timeout(
        cmd,
        Duration::from_secs(10),
        generation.unwrap_or("unknown"),
    )
    .map_err(|e| format!("Failed to run ffmpeg: {e}"))?;

    if !status.success() {
        return Err(format!("ffmpeg failed with status {status}"));
    }

    if tmp_path.exists() {
        std::fs::rename(&tmp_path, cache_path)
            .map_err(|e| format!("Move generated thumb failed: {e}"))?;
    }

    let dims = ImageReader::open(cache_path)
        .map_err(|e| format!("Read generated thumb failed: {e}"))?
        .with_guessed_format()
        .map_err(|e| format!("Guess format failed: {e}"))?
        .into_dimensions()
        .map_err(|e| format!("Read dimensions failed: {e}"))?;

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

fn kill_other_video_processes(current_gen: &str) {
    let mut map = VIDEO_PROCS.lock().expect("video procs poisoned");
    let mut to_kill: Vec<u32> = Vec::new();
    for (pid, (gen, _)) in map.iter() {
        if gen != current_gen {
            to_kill.push(*pid);
        }
    }
    for pid in to_kill {
        if let Some((_, mut child)) = map.remove(&pid) {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn run_with_timeout(
    mut cmd: Command,
    timeout: Duration,
    generation: &str,
) -> Result<std::process::ExitStatus, String> {
    use std::thread;
    use std::time::Instant;

    // Kill any stale video jobs from previous generations before starting this one.
    kill_other_video_processes(generation);

    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let child = cmd
        .spawn()
        .map_err(|e| format!("Spawn ffmpeg failed: {e}"))?;
    let pid = child.id();

    {
        let mut map = VIDEO_PROCS.lock().expect("video procs poisoned");
        map.insert(pid, (generation.to_string(), child));
    }

    let start = Instant::now();

    loop {
        let mut finished: Option<std::process::ExitStatus> = None;
        {
            let mut map = VIDEO_PROCS.lock().expect("video procs poisoned");
            if let Some((_, child)) = map.get_mut(&pid) {
                match child.try_wait() {
                    Ok(Some(status)) => finished = Some(status),
                    Ok(None) => {}
                    Err(e) => return Err(format!("Wait ffmpeg failed: {e}")),
                }
            } else {
                return Err("Video process missing".into());
            }
        }

        if let Some(status) = finished {
            let mut map = VIDEO_PROCS.lock().expect("video procs poisoned");
            map.remove(&pid);
            return Ok(status);
        }

        if start.elapsed() > timeout {
            let mut map = VIDEO_PROCS.lock().expect("video procs poisoned");
            if let Some((_, mut child)) = map.remove(&pid) {
                let _ = child.kill();
                let _ = child.wait();
            }
            return Err("ffmpeg timed out".into());
        }

        thread::sleep(Duration::from_millis(50));
    }
}

static VIDEO_PROCS: Lazy<std::sync::Mutex<HashMap<u32, (String, Child)>>> =
    Lazy::new(|| std::sync::Mutex::new(HashMap::new()));
