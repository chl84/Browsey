//! Prefer Photoshop's embedded preview, then its saved composite bitmap.
//! https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#50577409_74450
use super::{
    control::Control, CancelableReader, ThumbnailError, ThumbnailErrorCode, ThumbnailResult,
};
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits, RgbImage};
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::path::Path;

const MAX_PREVIEW_DIM: u32 = 4096;
const MAX_PREVIEW_BYTES: u64 = 16 * 1024 * 1024;

mod composite;

fn invalid() -> ThumbnailError {
    ThumbnailError::new(
        ThumbnailErrorCode::DecodeFailed,
        "Invalid PSD embedded preview",
    )
}

fn io_error(error: io::Error) -> ThumbnailError {
    let code = match error.kind() {
        io::ErrorKind::NotFound => ThumbnailErrorCode::NotFound,
        io::ErrorKind::PermissionDenied => ThumbnailErrorCode::PermissionDenied,
        _ => ThumbnailErrorCode::DecodeFailed,
    };
    ThumbnailError::new(code, format!("Read PSD preview failed: {error}"))
}

pub(super) fn decode_psd_preview(path: &Path, control: &Control) -> ThumbnailResult<DynamicImage> {
    control.check()?;
    let file = crate::fs_utils::open_regular_file_nofollow(path).map_err(io_error)?;
    let len = file.metadata().map_err(io_error)?.len();
    read_preview(file, len, control)
}

struct PreviewReader<'a, R> {
    inner: R,
    len: u64,
    position: u64,
    control: &'a Control,
}

impl<R: Read + Seek> PreviewReader<'_, R> {
    fn read_exact(&mut self, bytes: &mut [u8]) -> ThumbnailResult<()> {
        self.control.check()?;
        let end = self
            .position
            .checked_add(bytes.len() as u64)
            .ok_or_else(invalid)?;
        if end > self.len {
            return Err(invalid());
        }
        let result = self.inner.read_exact(bytes);
        self.control.check()?;
        result.map_err(io_error)?;
        self.position = end;
        Ok(())
    }

    fn bytes<const N: usize>(&mut self) -> ThumbnailResult<[u8; N]> {
        let mut bytes = [0; N];
        self.read_exact(&mut bytes)?;
        Ok(bytes)
    }

    fn u32(&mut self) -> ThumbnailResult<u32> {
        Ok(u32::from_be_bytes(self.bytes()?))
    }

    fn seek(&mut self, position: u64) -> ThumbnailResult<()> {
        self.control.check()?;
        if position > self.len {
            return Err(invalid());
        }
        self.inner
            .seek(SeekFrom::Start(position))
            .map_err(io_error)?;
        self.control.check()?;
        self.position = position;
        Ok(())
    }

    fn section_end(&self, len: u64, boundary: u64) -> ThumbnailResult<u64> {
        self.position
            .checked_add(len)
            .filter(|end| *end <= boundary)
            .ok_or_else(invalid)
    }
}

fn read_preview<R: Read + Seek>(
    input: R,
    len: u64,
    control: &Control,
) -> ThumbnailResult<DynamicImage> {
    let mut reader = PreviewReader {
        inner: input,
        len,
        position: 0,
        control,
    };
    let header = reader.bytes::<26>()?;
    if &header[..4] != b"8BPS"
        || u16::from_be_bytes([header[4], header[5]]) != 1
        || header[6..12] != [0; 6]
    {
        return Err(invalid());
    }
    let channels = u16::from_be_bytes([header[12], header[13]]);
    let height = u32::from_be_bytes(header[14..18].try_into().map_err(|_| invalid())?);
    let width = u32::from_be_bytes(header[18..22].try_into().map_err(|_| invalid())?);
    let depth = u16::from_be_bytes([header[22], header[23]]);
    let mode = u16::from_be_bytes([header[24], header[25]]);
    if !(1..=56).contains(&channels)
        || !(1..=30000).contains(&width)
        || !(1..=30000).contains(&height)
        || !matches!(depth, 1 | 8 | 16 | 32)
        || !matches!(mode, 0 | 1 | 2 | 3 | 4 | 7 | 8 | 9)
    {
        return Err(invalid());
    }
    let color_len = reader.u32()?;
    reader.seek(reader.section_end(u64::from(color_len), len)?)?;
    let resources_len = reader.u32()?;
    let resources_end = reader.section_end(u64::from(resources_len), len)?;
    let mut legacy = None;
    while reader.position < resources_end {
        // Every resource has a signature, ID, padded Pascal name and size.
        reader.section_end(12, resources_end)?;
        if &reader.bytes::<4>()? != b"8BIM" {
            return Err(invalid());
        }
        let id = u16::from_be_bytes(reader.bytes()?);
        let name_len = u64::from(reader.bytes::<1>()?[0]);
        let padded_name = name_len + (name_len + 1) % 2;
        let name_end = reader.section_end(padded_name, resources_end)?;
        reader.seek(name_end)?;
        reader.section_end(4, resources_end)?;
        let size = u64::from(reader.u32()?);
        let start = reader.position;
        let end = reader.section_end(size + size % 2, resources_end)?;
        if id == 1036 {
            return decode_resource(&mut reader, size, false);
        }
        if id == 1033 && legacy.is_none() {
            legacy = Some((start, size));
        }
        // Skip unrelated resources, including arbitrarily large metadata.
        reader.seek(end)?;
    }
    if let Some((start, size)) = legacy {
        reader.seek(start)?;
        return decode_resource(&mut reader, size, true);
    }
    composite::decode_composite(
        &mut reader,
        composite::Header {
            channels,
            width,
            height,
            depth,
            mode,
        },
    )
}

fn decode_resource<R: Read + Seek>(
    reader: &mut PreviewReader<'_, R>,
    size: u64,
    bgr: bool,
) -> ThumbnailResult<DynamicImage> {
    if size < 28 {
        return Err(invalid());
    }
    let format = reader.u32()?;
    let width = reader.u32()?;
    let height = reader.u32()?;
    let stride = reader.u32()?;
    let total = reader.u32()?;
    let encoded = reader.u32()?;
    let bits = u16::from_be_bytes(reader.bytes()?);
    let planes = u16::from_be_bytes(reader.bytes()?);
    if width == 0
        || height == 0
        || width > MAX_PREVIEW_DIM
        || height > MAX_PREVIEW_DIM
        || bits != 24
        || planes != 1
        || u64::from(stride) != (u64::from(width) * 24).div_ceil(32) * 4
        || u64::from(total) != u64::from(stride) * u64::from(height)
        || u64::from(total) > MAX_PREVIEW_BYTES
        || encoded == 0
        || u64::from(encoded) > MAX_PREVIEW_BYTES
        || u64::from(encoded) > size - 28
    {
        return Err(invalid());
    }
    if !matches!(format, 0 | 1) {
        return Err(ThumbnailError::new(
            ThumbnailErrorCode::UnsupportedFormat,
            "Unsupported PSD embedded preview encoding",
        ));
    }
    let mut bytes = vec![0; encoded as usize];
    reader.read_exact(&mut bytes)?;
    let mut image = if format == 1 {
        let input = CancelableReader {
            inner: Cursor::new(bytes),
            control: reader.control.clone(),
        };
        let mut image_reader = ImageReader::with_format(input, ImageFormat::Jpeg);
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_PREVIEW_DIM);
        limits.max_image_height = Some(MAX_PREVIEW_DIM);
        limits.max_alloc = Some(MAX_PREVIEW_BYTES);
        image_reader.limits(limits);
        let decoded = (|| {
            let decoder = image_reader.into_decoder()?;
            if decoder.dimensions() != (width, height) {
                return Err(image::ImageError::from(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "PSD preview dimensions mismatch",
                )));
            }
            DynamicImage::from_decoder(decoder)
        })();
        reader.control.check()?;
        decoded
            .map_err(|error| {
                ThumbnailError::new(
                    ThumbnailErrorCode::DecodeFailed,
                    format!("Decode PSD preview failed: {error}"),
                )
            })?
            .to_rgb8()
    } else {
        if encoded != total {
            return Err(invalid());
        }
        let row_bytes = width as usize * 3;
        let mut pixels = Vec::with_capacity(row_bytes * height as usize);
        for row in bytes.chunks_exact(stride as usize) {
            reader.control.check()?;
            pixels.extend_from_slice(&row[..row_bytes]);
        }
        RgbImage::from_raw(width, height, pixels).ok_or_else(invalid)?
    };
    // Photoshop 4's resource 1033 stores the thumbnail in BGR order.
    if bgr {
        for pixel in image.pixels_mut() {
            pixel.0.swap(0, 2);
        }
    }
    reader.control.check()?;
    Ok(DynamicImage::ImageRgb8(image))
}

#[cfg(test)]
mod tests;
