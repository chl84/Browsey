use super::render_pdf_thumbnail;
use crate::metadata::providers::pdf::collect;
use crate::pdfium_runtime::pdfium;
use image::GenericImageView;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn resources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("resources")
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        // Load the repository's bundled library before metadata's fallback
        // search can run. All tests exercise the same process-wide runtime.
        pdfium(Some(&resources())).expect("bundled PDFium must satisfy the pinned ABI");
        let path = std::env::temp_dir().join(format!(
            "browsey-pdf-regression-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn document(&self, width: u32, height: u32, rotation: u32, font: &str) -> PathBuf {
        let path = self.0.join("source.pdf");
        fs::write(&path, plain_pdf(width, height, rotation, font)).unwrap();
        path
    }

    fn thumbnail(&self, path: &Path, name: &str) -> (u32, u32) {
        let output = self.0.join(name);
        let dimensions = render_pdf_thumbnail(path, &output, 96, Some(&resources())).unwrap();
        assert_eq!(image::open(output).unwrap().dimensions(), dimensions);
        dimensions
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// Independent, minimal PDF writer: test inputs are not produced by PDFium.
fn plain_pdf(width: u32, height: u32, rotation: u32, font: &str) -> Vec<u8> {
    let content = "q 1 0 0 rg 20 20 150 150 re f Q\nBT /F1 40 Tf 20 400 Td (Browsey PDFium regression) Tj ET\n";
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Rotate {rotation} /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>"),
        format!("<< /Type /Font /Subtype /Type1 /BaseFont /{font} >>"),
        format!("<< /Length {} >>\nstream\n{content}endstream", content.len()),
        "<< /Title (Browsey PDF fixture) /Author (Browsey tests) /Subject (PDFium upgrade) >>".to_string(),
    ];
    let mut bytes = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = bytes.len();
    bytes.extend_from_slice(b"xref\n0 7\n0000000000 65535 f \n");
    for offset in offsets {
        bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend_from_slice(
        format!("trailer\n<< /Size 7 /Root 1 0 R /Info 6 0 R >>\nstartxref\n{xref}\n%%EOF\n")
            .as_bytes(),
    );
    bytes
}

#[test]
fn pdfium_bundle_loads_pinned_api_and_runtime_is_reused() {
    let fixture = Fixture::new();
    let first = pdfium(Some(&resources())).unwrap();
    // Once initialized, neither a missing resource directory nor another
    // consumer may attempt to rebind pdfium-render's global bindings.
    assert!(std::ptr::eq(first, pdfium(None).unwrap()));
    assert!(std::ptr::eq(first, pdfium(Some(&fixture.0)).unwrap()));
    #[cfg(target_os = "linux")]
    {
        let bundled = resources()
            .join("pdfium-linux-x64/lib/libpdfium.so")
            .canonicalize()
            .unwrap();
        assert!(
            fs::read_to_string("/proc/self/maps")
                .unwrap()
                .contains(bundled.to_str().unwrap()),
            "regressions must load the repository library, not a system fallback"
        );
    }
    let path = fixture.document(600, 800, 0, "Helvetica");
    assert_eq!(fixture.thumbnail(&path, "first.png"), (72, 96));
    assert_eq!(fixture.thumbnail(&path, "second.png"), (72, 96));
}

#[test]
fn pdfium_thumbnails_preserve_rotation_and_render_fonts() {
    let fixture = Fixture::new();
    for font in ["Helvetica", "Times-Roman", "Courier"] {
        for rotation in [0, 90, 180, 270] {
            let path = fixture.document(600, 800, rotation, font);
            let expected = if rotation % 180 == 0 {
                (72, 96)
            } else {
                (96, 72)
            };
            assert_eq!(fixture.thumbnail(&path, "render.png"), expected);
            let image = image::open(fixture.0.join("render.png")).unwrap().to_rgb8();
            assert!(
                image
                    .pixels()
                    .any(|pixel| pixel[0] > 180 && pixel[1] < 100 && pixel[2] < 100),
                "colored page content must render"
            );
            assert!(
                image
                    .pixels()
                    .any(|pixel| pixel[0] < 150 && pixel[1] < 150 && pixel[2] < 150),
                "{font} text must render at rotation {rotation}"
            );
        }
    }
}

#[test]
fn pdfium_thumbnails_keep_extreme_aspect_ratios_nonzero() {
    let fixture = Fixture::new();
    for (width, height, expected) in [(1, 10000, (1, 96)), (10000, 1, (96, 1))] {
        let path = fixture.document(width, height, 0, "Helvetica");
        assert_eq!(fixture.thumbnail(&path, "narrow.png"), expected);
    }
}

fn assert_metadata(path: &Path) {
    let sections = collect(path);
    let fields = &sections
        .iter()
        .find(|section| section.id == "pdf")
        .expect("PDF metadata missing")
        .fields;
    for (key, expected) in [
        ("page_count", "1"),
        ("pdf_version", "1.7"),
        ("title", "Browsey PDF fixture"),
        ("author", "Browsey tests"),
        ("subject", "PDFium upgrade"),
        ("encrypted", "No"),
    ] {
        assert_eq!(
            fields.iter().find(|field| field.key == key).unwrap().value,
            expected
        );
    }
}

#[test]
fn pdfium_metadata_survives_repeated_thumbnail_requests() {
    let fixture = Fixture::new();
    let path = fixture.document(600, 800, 0, "Helvetica");
    for _ in 0..10 {
        assert_metadata(&path);
        fixture.thumbnail(&path, "repeat.png");
        assert_metadata(&path);
    }
}

#[test]
fn pdfium_malformed_and_password_protected_files_fail_without_output() {
    let fixture = Fixture::new();
    let encrypted = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pdf/encrypted.pdf");
    // Establish that this is a valid encrypted PDF, not another corrupt input.
    let runtime = pdfium(None).unwrap();
    assert!(runtime
        .load_pdf_from_file(&encrypted, Some("wrong"))
        .is_err());
    assert!(runtime.load_pdf_from_file(&encrypted, Some("1234")).is_ok());
    let malformed = fixture.0.join("broken.pdf");
    for bytes in [
        b"".as_slice(),
        b"not a PDF",
        b"%PDF-1.7\n1 0 obj\n<< /Type /Catalog",
    ] {
        fs::write(&malformed, bytes).unwrap();
        let output = fixture.0.join("failed.png");
        assert!(render_pdf_thumbnail(&malformed, &output, 96, Some(&resources())).is_err());
        assert!(!output.exists());
        assert!(collect(&malformed).is_empty());
    }
    let output = fixture.0.join("encrypted.png");
    assert!(render_pdf_thumbnail(&encrypted, &output, 96, Some(&resources())).is_err());
    assert!(!output.exists());
    assert!(collect(&encrypted).is_empty());
}

#[test]
fn pdfium_parallel_thumbnails_and_metadata_share_one_runtime() {
    let fixture = Fixture::new();
    let path = fixture.document(600, 800, 0, "Helvetica");
    std::thread::scope(|scope| {
        for worker in 0..8 {
            let path = &path;
            let fixture = &fixture;
            scope.spawn(move || {
                for iteration in 0..8 {
                    assert_metadata(path);
                    fixture.thumbnail(path, &format!("{worker}-{iteration}.png"));
                }
            });
        }
    });
}
