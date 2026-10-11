use super::*;
use crate::commands::thumbnails::thumbnails_psd::read_preview;
use std::{
    io::Cursor,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

fn control() -> Control {
    Control::new(Arc::new(AtomicBool::new(false)), Duration::from_secs(5))
}

fn fixture(
    depth: u16,
    mode: u16,
    channels: u16,
    compression: u16,
    layer: &[u8],
    planes: &[Vec<u8>],
) -> Vec<u8> {
    let mut out = b"8BPS".to_vec();
    out.extend(1u16.to_be_bytes());
    out.extend([0; 6]);
    out.extend(channels.to_be_bytes());
    out.extend(2u32.to_be_bytes());
    out.extend(2u32.to_be_bytes());
    out.extend(depth.to_be_bytes());
    out.extend(mode.to_be_bytes());
    out.extend(0u32.to_be_bytes()); // No color mode data.
    out.extend(0u32.to_be_bytes()); // No embedded preview.
    out.extend((layer.len() as u32).to_be_bytes());
    out.extend(layer);
    out.extend(compression.to_be_bytes());
    if compression == 1 {
        let sample_bytes = usize::from(depth / 8);
        let mut rows = Vec::new();
        for plane in planes {
            for row in plane.chunks_exact(2 * sample_bytes) {
                // A PackBits literal packet followed by a legal no-op.
                let encoded = [vec![(row.len() - 1) as u8], row.to_vec(), vec![128]].concat();
                out.extend((encoded.len() as u16).to_be_bytes());
                rows.push(encoded);
            }
        }
        for row in rows {
            out.extend(row);
        }
    } else {
        for plane in planes {
            out.extend(plane);
        }
    }
    out
}

fn decode(bytes: &[u8]) -> ThumbnailResult<DynamicImage> {
    read_preview(Cursor::new(bytes), bytes.len() as u64, &control())
}

#[test]
fn raw_and_rle_rgb_composites_decode_big_endian_8_and_16_bit_planes() {
    for depth in [8, 16] {
        let planes: Vec<Vec<u8>> = [[10, 20, 30, 40], [50, 60, 70, 80], [90, 100, 110, 120]]
            .iter()
            .map(|values| {
                values
                    .iter()
                    .flat_map(|value| {
                        if depth == 16 {
                            vec![*value, 123]
                        } else {
                            vec![*value]
                        }
                    })
                    .collect()
            })
            .collect();
        for compression in [0, 1] {
            let bytes = fixture(depth, 3, 3, compression, &[], &planes);
            let image = decode(&bytes).unwrap().to_rgba8();
            assert_eq!(image.dimensions(), (2, 2));
            assert_eq!(
                image.as_raw(),
                &[10, 50, 90, 255, 20, 60, 100, 255, 30, 70, 110, 255, 40, 80, 120, 255]
            );
        }
    }
}

#[test]
fn grayscale_is_expanded_to_rgb_and_explicit_transparency_is_preserved() {
    for depth in [8, 16] {
        let gray: Vec<u8> = [0, 127, 128, 255]
            .iter()
            .flat_map(|value| {
                if depth == 16 {
                    vec![*value, 255]
                } else {
                    vec![*value]
                }
            })
            .collect();
        for compression in [0, 1] {
            let image = decode(&fixture(
                depth,
                1,
                1,
                compression,
                &[],
                std::slice::from_ref(&gray),
            ))
            .unwrap()
            .to_rgba8();
            assert_eq!(image.get_pixel(1, 0).0, [127, 127, 127, 255]);
            assert_eq!(image.get_pixel(1, 1).0, [255, 255, 255, 255]);
        }
    }
    let layer = [2u32.to_be_bytes().as_slice(), &(-1i16).to_be_bytes()].concat();
    let image = decode(&fixture(
        8,
        1,
        2,
        1,
        &layer,
        &[vec![10, 20, 30, 40], vec![0, 64, 128, 255]],
    ))
    .unwrap()
    .to_rgba8();
    assert_eq!(image.get_pixel(0, 1).0, [30, 30, 30, 128]);
}

#[test]
fn auxiliary_masks_are_not_treated_as_transparency() {
    let image = decode(&fixture(
        8,
        3,
        4,
        0,
        &[],
        &[vec![10; 4], vec![20; 4], vec![30; 4], vec![0; 4]],
    ))
    .unwrap()
    .to_rgba8();
    assert_eq!(image.get_pixel(0, 0).0, [10, 20, 30, 255]);
}

#[test]
fn packbits_handles_repeats_literals_and_noops_and_rejects_bad_row_lengths() {
    let mut out = [0; 5];
    unpack_row(&[128, 254, 10, 1, 20, 30, 128], &mut out, &control()).unwrap();
    assert_eq!(out, [10, 10, 10, 20, 30]);
    for bytes in [
        &[2, 10][..],
        &[255][..],
        &[250, 10][..],
        &[0, 10][..],
        &[128][..],
    ] {
        assert_eq!(
            unpack_row(bytes, &mut out, &control()).unwrap_err().code(),
            ThumbnailErrorCode::DecodeFailed
        );
    }
}

#[test]
fn truncated_composites_and_out_of_bounds_layer_sections_are_rejected() {
    for compression in [0, 1] {
        let bytes = fixture(
            16,
            3,
            3,
            compression,
            &[],
            &[vec![0; 8], vec![0; 8], vec![0; 8]],
        );
        for end in 34..bytes.len() {
            assert!(decode(&bytes[..end]).is_err(), "prefix {end}");
        }
        let mut malformed = bytes;
        malformed[34..38].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(decode(&malformed).is_err());
    }
}

#[test]
fn unsupported_composite_formats_and_large_canvases_keep_the_icon() {
    for (depth, mode, compression) in [(32, 3, 0), (8, 4, 0), (16, 3, 2), (16, 3, 3)] {
        let bytes = fixture(
            depth,
            mode,
            3,
            compression,
            &[],
            &[vec![0; 16], vec![0; 16], vec![0; 16]],
        );
        assert_eq!(
            decode(&bytes).unwrap_err().code(),
            ThumbnailErrorCode::UnsupportedFormat
        );
    }
    let mut bytes = fixture(16, 3, 3, 0, &[], &[]);
    bytes[14..18].copy_from_slice(&8000u32.to_be_bytes());
    bytes[18..22].copy_from_slice(&8000u32.to_be_bytes());
    assert_eq!(
        decode(&bytes).unwrap_err().code(),
        ThumbnailErrorCode::UnsupportedFormat
    );
}

#[test]
fn composite_layer_payload_is_skipped_and_rle_lengths_are_validated() {
    let mut layer = vec![0; 1024 * 1024];
    layer[..4].copy_from_slice(&0u32.to_be_bytes());
    let bytes = fixture(16, 3, 3, 1, &layer, &[vec![0; 8], vec![0; 8], vec![0; 8]]);
    decode(&bytes).unwrap();
    let mut malformed = fixture(16, 3, 3, 1, &[], &[vec![0; 8], vec![0; 8], vec![0; 8]]);
    malformed[40..42].copy_from_slice(&u16::MAX.to_be_bytes());
    assert!(decode(&malformed).is_err());
}

#[test]
fn composite_decoding_respects_cancellation_and_deadlines() {
    let bytes = fixture(16, 3, 3, 1, &[], &[vec![0; 8], vec![0; 8], vec![0; 8]]);
    let cancelled = Control::new(Arc::new(AtomicBool::new(true)), Duration::from_secs(5));
    assert_eq!(
        read_preview(Cursor::new(&bytes), bytes.len() as u64, &cancelled)
            .unwrap_err()
            .code(),
        ThumbnailErrorCode::Cancelled
    );
    let timed_out = Control::new(Arc::new(AtomicBool::new(false)), Duration::ZERO);
    assert_eq!(
        read_preview(Cursor::new(&bytes), bytes.len() as u64, &timed_out)
            .unwrap_err()
            .code(),
        ThumbnailErrorCode::TimedOut
    );
}
