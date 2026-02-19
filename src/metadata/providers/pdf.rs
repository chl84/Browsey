use crate::metadata::types::{ExtraMetadataField, ExtraMetadataSection};
use crate::metadata::{MetadataError, MetadataErrorCode, MetadataResult};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};

pub fn collect(path: &Path) -> Vec<ExtraMetadataSection> {
    let Ok(bindings) = load_pdfium_bindings() else {
        return Vec::new();
    };
    let pdfium = Pdfium::new(bindings);
    let Ok(doc) = pdfium.load_pdf_from_file(path, None) else {
        return Vec::new();
    };

    let mut fields: Vec<ExtraMetadataField> = Vec::new();

    fields.push(ExtraMetadataField::new(
        "page_count",
        "Pages",
        doc.pages().len().to_string(),
    ));
    fields.push(ExtraMetadataField::new(
        "pdf_version",
        "PDF version",
        version_label(doc.version()),
    ));

    let metadata = doc.metadata();
    for (key, label, tag_type) in [
        ("title", "Title", PdfDocumentMetadataTagType::Title),
        ("author", "Author", PdfDocumentMetadataTagType::Author),
        ("subject", "Subject", PdfDocumentMetadataTagType::Subject),
        ("keywords", "Keywords", PdfDocumentMetadataTagType::Keywords),
        ("creator", "Creator", PdfDocumentMetadataTagType::Creator),
        ("producer", "Producer", PdfDocumentMetadataTagType::Producer),
    ] {
        if let Some(tag) = metadata.get(tag_type) {
            let value = tag.value().trim();
            if !value.is_empty() {
                fields.push(ExtraMetadataField::new(key, label, value.to_string()));
            }
        }
    }

    if let Ok(revision) = doc.permissions().security_handler_revision() {
        fields.push(ExtraMetadataField::new(
            "security",
            "Security",
            security_label(revision),
        ));
        fields.push(ExtraMetadataField::new(
            "encrypted",
            "Encrypted",
            if matches!(revision, PdfSecurityHandlerRevision::Unprotected) {
                "No"
            } else {
                "Yes"
            },
        ));
    }

    vec![ExtraMetadataSection::new("pdf", "PDF").with_fields(fields)]
}

fn version_label(version: PdfDocumentVersion) -> String {
    match version {
        PdfDocumentVersion::Unset => "Unknown".to_string(),
        PdfDocumentVersion::Pdf1_0 => "1.0".to_string(),
        PdfDocumentVersion::Pdf1_1 => "1.1".to_string(),
        PdfDocumentVersion::Pdf1_2 => "1.2".to_string(),
        PdfDocumentVersion::Pdf1_3 => "1.3".to_string(),
        PdfDocumentVersion::Pdf1_4 => "1.4".to_string(),
        PdfDocumentVersion::Pdf1_5 => "1.5".to_string(),
        PdfDocumentVersion::Pdf1_6 => "1.6".to_string(),
        PdfDocumentVersion::Pdf1_7 => "1.7".to_string(),
        PdfDocumentVersion::Pdf2_0 => "2.0".to_string(),
        PdfDocumentVersion::Other(raw) => format!("{}.{:01}", raw / 10, raw % 10),
    }
}

fn security_label(revision: PdfSecurityHandlerRevision) -> String {
    match revision {
        PdfSecurityHandlerRevision::Unprotected => "Unprotected".to_string(),
        PdfSecurityHandlerRevision::Revision2 => "Revision 2".to_string(),
        PdfSecurityHandlerRevision::Revision3 => "Revision 3".to_string(),
        PdfSecurityHandlerRevision::Revision4 => "Revision 4".to_string(),
    }
}

fn load_pdfium_bindings() -> MetadataResult<Box<dyn PdfiumLibraryBindings>> {
    if let Ok(path) = std::env::var("PDFIUM_LIB_PATH") {
        if let Ok(bindings) = Pdfium::bind_to_library(&path) {
            return Ok(bindings);
        }
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            #[cfg(target_os = "linux")]
            candidates.push(dir.join("libpdfium.so"));
            #[cfg(target_os = "linux")]
            candidates.push(dir.join("resources/pdfium-linux-x64/lib/libpdfium.so"));
            #[cfg(target_os = "windows")]
            candidates.push(dir.join("resources/pdfium-win-x64/bin/pdfium.dll"));

            let proj_root = dir.parent().and_then(|p| p.parent()).unwrap_or(dir);
            #[cfg(target_os = "linux")]
            candidates.push(proj_root.join("resources/pdfium-linux-x64/lib/libpdfium.so"));
            #[cfg(target_os = "windows")]
            candidates.push(proj_root.join("resources/pdfium-win-x64/bin/pdfium.dll"));

            #[cfg(target_os = "windows")]
            candidates.push(dir.join("pdfium.dll"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        candidates.extend([
            PathBuf::from("/usr/lib64/libpdfium.so"),
            PathBuf::from("/usr/lib/libpdfium.so"),
            PathBuf::from("/usr/lib64/libdeepin-pdfium.so.1"),
            PathBuf::from("/usr/lib64/libdeepin-pdfium.so"),
        ]);
    }

    for candidate in candidates {
        if candidate.exists() {
            let candidate = candidate.to_string_lossy().to_string();
            if let Ok(bindings) = Pdfium::bind_to_library(&candidate) {
                return Ok(bindings);
            }
        }
    }

    Pdfium::bind_to_system_library().map_err(|error| {
        MetadataError::new(
            MetadataErrorCode::PdfiumLoadFailed,
            format!("Pdfium load failed: {error}"),
        )
    })
}
