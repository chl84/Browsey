use crate::metadata::types::{ExtraMetadataField, ExtraMetadataSection};
use image::{ColorType, ImageDecoder, ImageReader};
use resvg::usvg::{Options, Tree};
use std::path::Path;

pub fn collect(path: &Path) -> Vec<ExtraMetadataSection> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if ext == "svg" {
        return collect_svg(path);
    }
    collect_raster(path)
}

fn collect_raster(path: &Path) -> Vec<ExtraMetadataSection> {
    let Ok(reader) = ImageReader::open(path) else {
        return Vec::new();
    };
    let Ok(reader) = reader.with_guessed_format() else {
        return Vec::new();
    };
    let format = reader.format();
    let Ok(mut decoder) = reader.into_decoder() else {
        return Vec::new();
    };

    let (width, height) = decoder.dimensions();
    let color_type = decoder.color_type();
    let channels = color_type.channel_count().max(1) as u16;
    let bits_per_channel = color_type.bits_per_pixel() / channels;
    let orientation = decoder.orientation().ok();

    let mut fields: Vec<ExtraMetadataField> = Vec::new();
    fields.push(ExtraMetadataField::new(
        "resolution",
        "Resolution",
        format!("{width} x {height}"),
    ));
    fields.push(ExtraMetadataField::new(
        "color_model",
        "Color model",
        color_type_label(color_type),
    ));
    fields.push(ExtraMetadataField::new(
        "bit_depth",
        "Bit depth",
        format!("{bits_per_channel}-bit/channel"),
    ));

    if let Some(fmt) = format {
        fields.push(ExtraMetadataField::new(
            "format",
            "Image format",
            format!("{fmt:?}"),
        ));
    }

    if let Some(orientation) = orientation {
        let label = orientation_label(orientation);
        if !label.is_empty() {
            fields.push(ExtraMetadataField::new("orientation", "Orientation", label));
        }
    }

    vec![ExtraMetadataSection::new("image", "Image").with_fields(fields)]
}

fn collect_svg(path: &Path) -> Vec<ExtraMetadataSection> {
    let Ok(data) = std::fs::read(path) else {
        return Vec::new();
    };
    let mut options = Options::default();
    options.resources_dir = path.parent().map(|p| p.to_path_buf());
    let Ok(tree) = Tree::from_data(&data, &options) else {
        return Vec::new();
    };

    let size = tree.size();
    if size.width() <= 0.0 || size.height() <= 0.0 {
        return Vec::new();
    }

    let mut fields: Vec<ExtraMetadataField> = Vec::new();
    fields.push(ExtraMetadataField::new(
        "resolution",
        "Resolution",
        format!("{:.0} x {:.0}", size.width(), size.height()),
    ));
    fields.push(ExtraMetadataField::new("format", "Image format", "SVG"));

    vec![ExtraMetadataSection::new("image", "Image").with_fields(fields)]
}

fn color_type_label(color_type: ColorType) -> String {
    match color_type {
        ColorType::L8 => "Grayscale (8-bit)".to_string(),
        ColorType::La8 => "Grayscale + alpha (8-bit)".to_string(),
        ColorType::Rgb8 => "RGB (8-bit)".to_string(),
        ColorType::Rgba8 => "RGBA (8-bit)".to_string(),
        ColorType::L16 => "Grayscale (16-bit)".to_string(),
        ColorType::La16 => "Grayscale + alpha (16-bit)".to_string(),
        ColorType::Rgb16 => "RGB (16-bit)".to_string(),
        ColorType::Rgba16 => "RGBA (16-bit)".to_string(),
        ColorType::Rgb32F => "RGB (32-bit float)".to_string(),
        ColorType::Rgba32F => "RGBA (32-bit float)".to_string(),
        _ => format!("{color_type:?}"),
    }
}

fn orientation_label(orientation: image::metadata::Orientation) -> String {
    use image::metadata::Orientation;
    match orientation {
        Orientation::NoTransforms => String::new(),
        Orientation::Rotate90 => "Rotate 90".to_string(),
        Orientation::Rotate180 => "Rotate 180".to_string(),
        Orientation::Rotate270 => "Rotate 270".to_string(),
        Orientation::FlipHorizontal => "Flip horizontal".to_string(),
        Orientation::FlipVertical => "Flip vertical".to_string(),
        Orientation::Rotate90FlipH => "Rotate 90 + flip horizontal".to_string(),
        Orientation::Rotate270FlipH => "Rotate 270 + flip horizontal".to_string(),
    }
}
