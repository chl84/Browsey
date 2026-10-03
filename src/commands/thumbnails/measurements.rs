use super::*;
use crate::performance_fixture::{report, Fixture};
use std::time::Instant;

#[test]
#[ignore = "Generates mixed 2560x1440 images and measures real decode/cache paths"]
fn mixed_thumbnail_workloads() {
    let fixture = Fixture::new("thumbnails");
    let cache = fixture.0.join("cache");
    fs::create_dir(&cache).unwrap();
    let image = RgbImage::from_fn(2560, 1440, |x, y| {
        image::Rgb([
            (x.wrapping_mul(31) ^ y) as u8,
            (y.wrapping_mul(17) ^ x) as u8,
            (x + y) as u8,
        ])
    });
    let mut inputs = Vec::new();
    for (extension, format) in [
        ("jpg", ImageFormat::Jpeg),
        ("png", ImageFormat::Png),
        ("webp", ImageFormat::WebP),
        ("gif", ImageFormat::Gif),
        ("bmp", ImageFormat::Bmp),
        ("tiff", ImageFormat::Tiff),
    ] {
        let input = fixture.0.join(format!("input.{extension}"));
        image.save_with_format(&input, format).unwrap();
        inputs.push(input);
    }
    let svg = fixture.0.join("input.svg");
    fs::write(&svg, r#"<svg xmlns="http://www.w3.org/2000/svg" width="2560" height="1440"><rect width="2560" height="1440" fill="red"/></svg>"#).unwrap();
    inputs.push(svg);
    let pdf = fixture.0.join("input.pdf");
    fs::write(
        &pdf,
        thumbnails_pdf::tests::plain_pdf(600, 800, 0, "Helvetica"),
    )
    .unwrap();
    inputs.push(pdf);
    let resource_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources");
    for input in inputs {
        for dimension in [96, 384] {
            let output = cache.join(format!(
                "{}-{dimension}.png",
                input.extension().unwrap().to_str().unwrap()
            ));
            let mut decode_ms = Vec::new();
            let mut hit_ms = Vec::new();
            for _ in 0..5 {
                let control = control::Control::new(Default::default(), Duration::from_secs(30));
                let start = Instant::now();
                let result = generate_thumbnail(
                    &input,
                    &output,
                    dimension,
                    Some(&resource_dir),
                    None,
                    &control,
                )
                .unwrap();
                decode_ms.push(start.elapsed().as_secs_f64() * 1000.0);
                assert!(result.width > 0 && result.height > 0);
                assert!(result.width.max(result.height) <= dimension);
                let start = Instant::now();
                assert!(cached_response(&output).unwrap().cached);
                hit_ms.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            let extra = serde_json::json!({
                "format": input.extension().unwrap().to_str().unwrap(), "maxDimension": dimension,
                "sourceBytes": fs::metadata(&input).unwrap().len(), "cacheBytes": fs::metadata(&output).unwrap().len(),
                "scope": "production decoder/encoder and cache-hit functions; excludes admission, IPC and display",
            });
            report("thumbnail-generation-core", &decode_ms, extra.clone());
            report("thumbnail-cache-hit-core", &hit_ms, extra);
        }
    }
    let start = Instant::now();
    let (files, bytes) = thumbnail_cache_stats(&cache).unwrap();
    assert_eq!(files, 16);
    report(
        "thumbnail-fixture-cache-inventory",
        &[start.elapsed().as_secs_f64() * 1000.0],
        serde_json::json!({"cacheFiles": files, "cacheLogicalBytes": bytes}),
    );
}
