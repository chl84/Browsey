use crate::metadata::types::{ExtraMetadataField, ExtraMetadataSection};
use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use tar::Archive;
use xz2::read::XzDecoder;
use zip::ZipArchive;
use zstd::stream::read::Decoder as ZstdDecoder;

pub fn collect(path: &Path) -> Vec<ExtraMetadataSection> {
    let Some(kind) = classify_archive(path) else {
        return Vec::new();
    };

    let mut fields: Vec<ExtraMetadataField> = Vec::new();
    fields.push(ExtraMetadataField::new(
        "archive_format",
        "Archive format",
        kind_label(kind),
    ));

    let metrics = match kind {
        ArchiveKind::Zip => zip_metrics(path).ok(),
        ArchiveKind::Tar
        | ArchiveKind::TarGz
        | ArchiveKind::TarBz2
        | ArchiveKind::TarXz
        | ArchiveKind::TarZstd => tar_metrics(path, kind).ok(),
        _ => None,
    };

    if let Some((entries, uncompressed)) = metrics {
        fields.push(ExtraMetadataField::new(
            "entries",
            "Entries",
            entries.to_string(),
        ));
        fields.push(ExtraMetadataField::new(
            "uncompressed_size",
            "Uncompressed size",
            format_bytes(uncompressed),
        ));
    }

    vec![ExtraMetadataSection::new("archive", "Archive").with_fields(fields)]
}

#[derive(Copy, Clone)]
enum ArchiveKind {
    Zip,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    TarZstd,
    SevenZ,
    Rar,
    Gz,
    Bz2,
    Xz,
    Zstd,
}

fn classify_archive(path: &Path) -> Option<ArchiveKind> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        return Some(ArchiveKind::TarGz);
    }
    if name.ends_with(".tar.bz2") || name.ends_with(".tbz2") {
        return Some(ArchiveKind::TarBz2);
    }
    if name.ends_with(".tar.xz") || name.ends_with(".txz") {
        return Some(ArchiveKind::TarXz);
    }
    if name.ends_with(".tar.zst") || name.ends_with(".tzst") {
        return Some(ArchiveKind::TarZstd);
    }
    if name.ends_with(".tar") {
        return Some(ArchiveKind::Tar);
    }
    if name.ends_with(".zip") {
        return Some(ArchiveKind::Zip);
    }
    if name.ends_with(".7z") {
        return Some(ArchiveKind::SevenZ);
    }
    if name.ends_with(".rar") {
        return Some(ArchiveKind::Rar);
    }
    if name.ends_with(".gz") {
        return Some(ArchiveKind::Gz);
    }
    if name.ends_with(".bz2") {
        return Some(ArchiveKind::Bz2);
    }
    if name.ends_with(".xz") {
        return Some(ArchiveKind::Xz);
    }
    if name.ends_with(".zst") {
        return Some(ArchiveKind::Zstd);
    }
    None
}

fn kind_label(kind: ArchiveKind) -> &'static str {
    match kind {
        ArchiveKind::Zip => "ZIP",
        ArchiveKind::Tar => "TAR",
        ArchiveKind::TarGz => "TAR.GZ",
        ArchiveKind::TarBz2 => "TAR.BZ2",
        ArchiveKind::TarXz => "TAR.XZ",
        ArchiveKind::TarZstd => "TAR.ZST",
        ArchiveKind::SevenZ => "7Z",
        ArchiveKind::Rar => "RAR",
        ArchiveKind::Gz => "GZIP stream",
        ArchiveKind::Bz2 => "BZIP2 stream",
        ArchiveKind::Xz => "XZ stream",
        ArchiveKind::Zstd => "ZSTD stream",
    }
}

fn zip_metrics(path: &Path) -> Result<(u64, u64), String> {
    let file = File::open(path).map_err(|e| format!("Failed to open zip: {e}"))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("Failed to read zip: {e}"))?;

    let mut entries = 0u64;
    let mut uncompressed = 0u64;
    for idx in 0..archive.len() {
        let file = archive
            .by_index(idx)
            .map_err(|e| format!("Failed to read zip entry: {e}"))?;
        if file.is_dir() {
            continue;
        }
        entries = entries.saturating_add(1);
        uncompressed = uncompressed.saturating_add(file.size());
    }
    Ok((entries, uncompressed))
}

fn tar_metrics(path: &Path, kind: ArchiveKind) -> Result<(u64, u64), String> {
    let file = File::open(path).map_err(|e| format!("Failed to open archive: {e}"))?;
    let reader = BufReader::new(file);
    match kind {
        ArchiveKind::Tar => tar_metrics_with_reader(reader),
        ArchiveKind::TarGz => tar_metrics_with_reader(GzDecoder::new(reader)),
        ArchiveKind::TarBz2 => tar_metrics_with_reader(BzDecoder::new(reader)),
        ArchiveKind::TarXz => tar_metrics_with_reader(XzDecoder::new(reader)),
        ArchiveKind::TarZstd => {
            let decoder =
                ZstdDecoder::new(reader).map_err(|e| format!("Failed to read zstd stream: {e}"))?;
            tar_metrics_with_reader(decoder)
        }
        _ => Err("Unsupported tar variant".to_string()),
    }
}

fn tar_metrics_with_reader<R: Read>(reader: R) -> Result<(u64, u64), String> {
    let mut archive = Archive::new(reader);
    let mut entries = 0u64;
    let mut uncompressed = 0u64;

    let iter = archive
        .entries()
        .map_err(|e| format!("Failed to iterate tar entries: {e}"))?;
    for entry_result in iter {
        let entry = entry_result.map_err(|e| format!("Failed to read tar entry: {e}"))?;
        let header = entry.header();
        if header.entry_type().is_dir() {
            continue;
        }
        entries = entries.saturating_add(1);
        let size = header
            .size()
            .map_err(|e| format!("Failed to read tar entry size: {e}"))?;
        uncompressed = uncompressed.saturating_add(size);
    }
    Ok((entries, uncompressed))
}

fn format_bytes(bytes: u64) -> String {
    const KI: f64 = 1024.0;
    const MI: f64 = KI * 1024.0;
    const GI: f64 = MI * 1024.0;
    let value = bytes as f64;
    if value >= GI {
        format!("{:.2} GiB", value / GI)
    } else if value >= MI {
        format!("{:.2} MiB", value / MI)
    } else if value >= KI {
        format!("{:.2} KiB", value / KI)
    } else {
        format!("{bytes} B")
    }
}
