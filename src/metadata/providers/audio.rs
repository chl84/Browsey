use super::media_probe::{
    format_bits_per_second, format_channels, format_duration, format_sample_rate, probe,
};
use crate::metadata::types::{ExtraMetadataField, ExtraMetadataSection};
use std::path::Path;

pub fn collect(path: &Path) -> Vec<ExtraMetadataSection> {
    let Some(probe) = probe(path) else {
        return Vec::new();
    };
    let Some(audio) = probe.first_audio_stream() else {
        return Vec::new();
    };

    let mut fields: Vec<ExtraMetadataField> = Vec::new();

    if let Some(codec) = audio
        .codec_name
        .as_deref()
        .or(audio.codec_long_name.as_deref())
    {
        fields.push(ExtraMetadataField::new("codec", "Codec", codec.to_string()));
    }

    if let Some(duration) = audio.duration_secs.or(probe.duration_secs) {
        fields.push(ExtraMetadataField::new(
            "duration",
            "Duration",
            format_duration(duration),
        ));
    }

    if let Some(rate) = audio.sample_rate {
        fields.push(ExtraMetadataField::new(
            "sample_rate",
            "Sample rate",
            format_sample_rate(rate),
        ));
    }

    if let Some(channels) = audio.channels {
        fields.push(ExtraMetadataField::new(
            "channels",
            "Channels",
            format_channels(channels),
        ));
    }

    if let Some(bit_rate) = audio.bit_rate.or(probe.bit_rate) {
        fields.push(ExtraMetadataField::new(
            "bit_rate",
            "Bit rate",
            format_bits_per_second(bit_rate),
        ));
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

    vec![ExtraMetadataSection::new("audio", "Audio").with_fields(fields)]
}
