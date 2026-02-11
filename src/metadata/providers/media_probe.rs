use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct MediaStream {
    pub codec_type: Option<String>,
    pub codec_name: Option<String>,
    pub codec_long_name: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub avg_frame_rate: Option<String>,
    pub r_frame_rate: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
    pub bit_rate: Option<u64>,
    pub duration_secs: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct MediaProbe {
    pub format_name: Option<String>,
    pub duration_secs: Option<f64>,
    pub bit_rate: Option<u64>,
    pub streams: Vec<MediaStream>,
}

impl MediaProbe {
    pub fn first_video_stream(&self) -> Option<&MediaStream> {
        self.streams
            .iter()
            .find(|stream| stream.codec_type.as_deref() == Some("video"))
    }

    pub fn first_audio_stream(&self) -> Option<&MediaStream> {
        self.streams
            .iter()
            .find(|stream| stream.codec_type.as_deref() == Some("audio"))
    }
}

pub fn probe(path: &Path) -> Option<MediaProbe> {
    let ffprobe = resolve_ffprobe_bin()?;
    let output = Command::new(ffprobe)
        .arg("-v")
        .arg("error")
        .arg("-print_format")
        .arg("json")
        .arg("-show_format")
        .arg("-show_streams")
        .arg(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let parsed: Value = serde_json::from_slice(&output.stdout).ok()?;
    Some(parse_probe(&parsed))
}

pub fn parse_frame_rate(raw: &str) -> Option<f64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some((num, den)) = trimmed.split_once('/') {
        let n = num.trim().parse::<f64>().ok()?;
        let d = den.trim().parse::<f64>().ok()?;
        if d <= 0.0 {
            return None;
        }
        let fps = n / d;
        if fps > 0.0 {
            return Some(fps);
        }
        return None;
    }
    let fps = trimmed.parse::<f64>().ok()?;
    if fps > 0.0 {
        Some(fps)
    } else {
        None
    }
}

pub fn format_duration(secs: f64) -> String {
    let safe_secs = secs.max(0.0);
    let total = safe_secs.round() as u64;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

pub fn format_bits_per_second(bits_per_sec: u64) -> String {
    if bits_per_sec >= 1_000_000_000 {
        return format!("{:.2} Gb/s", bits_per_sec as f64 / 1_000_000_000.0);
    }
    if bits_per_sec >= 1_000_000 {
        return format!("{:.1} Mb/s", bits_per_sec as f64 / 1_000_000.0);
    }
    if bits_per_sec >= 1_000 {
        return format!("{:.0} kb/s", bits_per_sec as f64 / 1_000.0);
    }
    format!("{bits_per_sec} b/s")
}

pub fn format_sample_rate(sample_rate_hz: u32) -> String {
    let khz = sample_rate_hz as f64 / 1000.0;
    if (khz - khz.round()).abs() < 0.05 {
        format!("{:.0} kHz", khz)
    } else {
        format!("{:.1} kHz", khz)
    }
}

pub fn format_channels(channels: u32) -> String {
    match channels {
        1 => "Mono".to_string(),
        2 => "Stereo".to_string(),
        n => format!("{n} channels"),
    }
}

fn parse_probe(root: &Value) -> MediaProbe {
    let mut out = MediaProbe::default();

    if let Some(format_obj) = root.get("format").and_then(Value::as_object) {
        out.format_name = get_string(format_obj.get("format_name"));
        out.duration_secs = parse_f64(format_obj.get("duration"));
        out.bit_rate = parse_u64(format_obj.get("bit_rate"));
    }

    if let Some(streams) = root.get("streams").and_then(Value::as_array) {
        out.streams = streams.iter().map(parse_stream).collect();
    }

    out
}

fn parse_stream(stream: &Value) -> MediaStream {
    MediaStream {
        codec_type: get_string(stream.get("codec_type")),
        codec_name: get_string(stream.get("codec_name")),
        codec_long_name: get_string(stream.get("codec_long_name")),
        width: parse_u32(stream.get("width")),
        height: parse_u32(stream.get("height")),
        avg_frame_rate: get_string(stream.get("avg_frame_rate")),
        r_frame_rate: get_string(stream.get("r_frame_rate")),
        sample_rate: parse_u32(stream.get("sample_rate")),
        channels: parse_u32(stream.get("channels")),
        bit_rate: parse_u64(stream.get("bit_rate")),
        duration_secs: parse_f64(stream.get("duration")),
    }
}

fn get_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

fn parse_u64(value: Option<&Value>) -> Option<u64> {
    match value {
        Some(Value::Number(n)) => n.as_u64(),
        Some(Value::String(s)) => s.trim().parse::<u64>().ok(),
        _ => None,
    }
}

fn parse_u32(value: Option<&Value>) -> Option<u32> {
    parse_u64(value).and_then(|v| u32::try_from(v).ok())
}

fn parse_f64(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn resolve_ffprobe_bin() -> Option<PathBuf> {
    let env_probe = std::env::var("FFPROBE_BIN").ok().map(PathBuf::from);

    let configured_probe = configured_ffmpeg_binary()
        .as_deref()
        .and_then(derive_ffprobe_from_ffmpeg);
    let env_derived_probe = std::env::var("FFMPEG_BIN")
        .ok()
        .map(PathBuf::from)
        .as_deref()
        .and_then(derive_ffprobe_from_ffmpeg);

    crate::binary_resolver::resolve_binary_with_overrides(
        "ffprobe",
        [env_probe, configured_probe, env_derived_probe]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>(),
    )
}

fn configured_ffmpeg_binary() -> Option<PathBuf> {
    let conn = crate::db::open().ok()?;
    let raw = crate::db::get_setting_string(&conn, "ffmpegPath")
        .ok()
        .flatten()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let candidate = PathBuf::from(trimmed);
    if !candidate.exists() {
        return None;
    }
    candidate.canonicalize().ok().or(Some(candidate))
}

fn derive_ffprobe_from_ffmpeg(ffmpeg_path: &Path) -> Option<PathBuf> {
    let ffprobe_name = match ffmpeg_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_ascii_lowercase())
    {
        Some(name) if name.ends_with(".exe") => "ffprobe.exe",
        _ => "ffprobe",
    };
    let candidate = ffmpeg_path.with_file_name(ffprobe_name);
    if candidate.exists() {
        Some(candidate)
    } else {
        None
    }
}
