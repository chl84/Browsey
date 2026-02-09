use super::media_probe::{format_bits_per_second, format_duration, parse_frame_rate, probe};
use crate::metadata::types::{ExtraMetadataField, ExtraMetadataSection};
use std::path::Path;

pub fn collect(path: &Path) -> Vec<ExtraMetadataSection> {
    let Some(probe) = probe(path) else {
        return Vec::new();
    };
    let Some(video) = probe.first_video_stream() else {
        return Vec::new();
    };
    let audio = probe.first_audio_stream();

    let mut fields: Vec<ExtraMetadataField> = Vec::new();

    if let (Some(width), Some(height)) = (video.width, video.height) {
        fields.push(ExtraMetadataField::new(
            "resolution",
            "Resolution",
            format!("{width} x {height}"),
        ));
    }

    if let Some(codec) = video
        .codec_name
        .as_deref()
        .or(video.codec_long_name.as_deref())
    {
        fields.push(ExtraMetadataField::new(
            "video_codec",
            "Video codec",
            codec.to_string(),
        ));
    }

    let fps = video
        .avg_frame_rate
        .as_deref()
        .and_then(parse_frame_rate)
        .or_else(|| video.r_frame_rate.as_deref().and_then(parse_frame_rate));
    if let Some(fps) = fps {
        fields.push(ExtraMetadataField::new(
            "frame_rate",
            "Frame rate",
            format!("{fps:.2} fps"),
        ));
    }

    if let Some(duration) = video.duration_secs.or(probe.duration_secs) {
        fields.push(ExtraMetadataField::new(
            "duration",
            "Duration",
            format_duration(duration),
        ));
    }

    if let Some(bit_rate) = video.bit_rate.or(probe.bit_rate) {
        fields.push(ExtraMetadataField::new(
            "bit_rate",
            "Bit rate",
            format_bits_per_second(bit_rate),
        ));
    }

    if let Some(audio_stream) = audio {
        if let Some(codec) = audio_stream
            .codec_name
            .as_deref()
            .or(audio_stream.codec_long_name.as_deref())
        {
            fields.push(ExtraMetadataField::new(
                "audio_codec",
                "Audio codec",
                codec.to_string(),
            ));
        }
    }

    if let Some(container) = probe.format_name.as_deref() {
        fields.push(ExtraMetadataField::new(
            "container",
            "Container",
            container.to_string(),
        ));
    }

    if fields.is_empty() {
        return Vec::new();
    }

    vec![ExtraMetadataSection::new("video", "Video").with_fields(fields)]
}
