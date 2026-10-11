//! Decode the saved composite only. Photoshop layers are never rendered here.
use super::{
    Control, DynamicImage, PreviewReader, ThumbnailError, ThumbnailErrorCode, ThumbnailResult,
};
use image::RgbaImage;
use std::io::{Read, Seek};

const MAX_COMPOSITE_BYTES: u64 = 64 * 1024 * 1024;

pub(super) struct Header {
    pub(super) channels: u16,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) depth: u16,
    pub(super) mode: u16,
}

fn invalid() -> ThumbnailError {
    ThumbnailError::new(
        ThumbnailErrorCode::DecodeFailed,
        "Invalid PSD composite image",
    )
}

fn unsupported() -> ThumbnailError {
    ThumbnailError::new(
        ThumbnailErrorCode::UnsupportedFormat,
        "PSD composite preview requires 8/16-bit RGB or grayscale with raw/RLE compression",
    )
}

pub(super) fn decode_composite<R: Read + Seek>(
    reader: &mut PreviewReader<'_, R>,
    header: Header,
) -> ThumbnailResult<DynamicImage> {
    let color_channels = match header.mode {
        1 => 1usize,
        3 => 3usize,
        _ => return Err(unsupported()),
    };
    if !matches!(header.depth, 8 | 16) || usize::from(header.channels) < color_channels {
        return Err(unsupported());
    }
    let pixel_count = u64::from(header.width) * u64::from(header.height);
    if header.width > crate::commands::thumbnails::MAX_SOURCE_DIM
        || header.height > crate::commands::thumbnails::MAX_SOURCE_DIM
        || pixel_count * 4 > MAX_COMPOSITE_BYTES
    {
        return Err(ThumbnailError::new(
            ThumbnailErrorCode::UnsupportedFormat,
            "PSD composite image exceeds thumbnail decode limits",
        ));
    }

    let layer_size = reader.u32()?;
    let layer_end = reader.section_end(u64::from(layer_size), reader.len)?;
    // A negative layer count identifies composite transparency. Extra channels
    // otherwise hold saved selections/masks and must not be mistaken for alpha.
    let mut has_alpha = false;
    if layer_size != 0 {
        if layer_size < 4 {
            return Err(invalid());
        }
        let info_size = reader.u32()?;
        reader.section_end(u64::from(info_size), layer_end)?;
        if info_size != 0 {
            if info_size < 2 {
                return Err(invalid());
            }
            has_alpha = i16::from_be_bytes(reader.bytes()?) < 0;
        }
    }
    reader.seek(layer_end)?;
    if has_alpha && usize::from(header.channels) <= color_channels {
        return Err(invalid());
    }
    let compression = u16::from_be_bytes(reader.bytes()?);
    if !matches!(compression, 0 | 1) {
        return Err(unsupported());
    }

    let sample_bytes = usize::from(header.depth / 8);
    let row_bytes = header.width as usize * sample_bytes;
    let height = header.height as usize;
    let row_lengths = if compression == 1 {
        let count = height * usize::from(header.channels);
        let mut bytes = vec![0; count * 2];
        reader.read_exact(&mut bytes)?;
        let lengths: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
            .collect();
        let total: u64 = lengths.iter().map(|length| u64::from(*length)).sum();
        reader.section_end(total, reader.len)?;
        lengths
    } else {
        reader.section_end(
            pixel_count * u64::from(header.channels) * sample_bytes as u64,
            reader.len,
        )?;
        Vec::new()
    };

    let mut pixels = vec![255; pixel_count as usize * 4];
    let mut row = vec![0; row_bytes];
    let mut encoded = Vec::new();
    let needed_channels = color_channels + usize::from(has_alpha);
    for channel in 0..needed_channels {
        for y in 0..height {
            reader.control.check()?;
            if compression == 1 {
                encoded.resize(usize::from(row_lengths[channel * height + y]), 0);
                reader.read_exact(&mut encoded)?;
                unpack_row(&encoded, &mut row, reader.control)?;
            } else {
                reader.read_exact(&mut row)?;
            }
            let output_row =
                &mut pixels[y * header.width as usize * 4..(y + 1) * header.width as usize * 4];
            for (sample, pixel) in row
                .chunks_exact(sample_bytes)
                .zip(output_row.as_chunks_mut::<4>().0.iter_mut())
            {
                // The high byte reduces big-endian 16-bit samples to 8-bit.
                let value = sample[0];
                if channel == color_channels {
                    pixel[3] = value;
                } else if color_channels == 1 {
                    pixel[..3].fill(value);
                } else {
                    pixel[channel] = value;
                }
            }
        }
    }
    reader.control.check()?;
    let image = RgbaImage::from_raw(header.width, header.height, pixels).ok_or_else(invalid)?;
    Ok(DynamicImage::ImageRgba8(image))
}

fn unpack_row(encoded: &[u8], decoded: &mut [u8], control: &Control) -> ThumbnailResult<()> {
    control.check()?;
    let mut input = 0;
    let mut output = 0;
    while input < encoded.len() {
        let code = encoded[input] as i8;
        input += 1;
        if code >= 0 {
            let count = code as usize + 1;
            let source = encoded.get(input..input + count).ok_or_else(invalid)?;
            let destination = decoded
                .get_mut(output..output + count)
                .ok_or_else(invalid)?;
            destination.copy_from_slice(source);
            input += count;
            output += count;
        } else if code != i8::MIN {
            let count = (1i16 - i16::from(code)) as usize;
            let value = *encoded.get(input).ok_or_else(invalid)?;
            decoded
                .get_mut(output..output + count)
                .ok_or_else(invalid)?
                .fill(value);
            input += 1;
            output += count;
        }
    }
    control.check()?;
    if output != decoded.len() {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
