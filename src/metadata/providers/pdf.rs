use crate::metadata::types::{ExtraMetadataField, ExtraMetadataSection};
use crate::metadata::{MetadataError, MetadataErrorCode, MetadataResult};
use pdfium_render::prelude::*;
use std::path::Path;

pub fn collect(path: &Path) -> Vec<ExtraMetadataSection> {
    let Ok(pdfium) = load_pdfium() else {
        return Vec::new();
    };
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

fn load_pdfium() -> MetadataResult<&'static Pdfium> {
    crate::pdfium_runtime::pdfium(None).map_err(|error| {
        MetadataError::new(
            MetadataErrorCode::PdfiumLoadFailed,
            format!("Pdfium load failed: {error}"),
        )
    })
}
