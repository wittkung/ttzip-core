// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.

//! Integration tests for SIMD image decoding and thumbnail generation pipeline.

use crc32fast::Hasher;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::Write;
use ttzip_engine::standards::image_pipeline::{
    calculate_aspect_dimensions, decode_image_rgba, generate_thumbnail, resize_rgba,
    DecodedImageRgba, ImagePipelineError, ThumbnailFilter,
};
use ttzip_engine::uniffi_api::image::{
    decode_image_rgba_from_memory, generate_thumbnail_from_memory, ThumbnailSamplingFilter,
};

fn make_test_bmp(w: u32, h: u32) -> Vec<u8> {
    let row_size = (w * 3).div_ceil(4) * 4;
    let pixel_data_size = row_size * h;
    let file_size = 54 + pixel_data_size;
    let mut buf = Vec::with_capacity(file_size as usize);
    buf.extend_from_slice(b"BM");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(&[0, 0, 0, 0]);
    buf.extend_from_slice(&54u32.to_le_bytes());
    buf.extend_from_slice(&40u32.to_le_bytes());
    buf.extend_from_slice(&(w as i32).to_le_bytes());
    buf.extend_from_slice(&(h as i32).to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&24u16.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&pixel_data_size.to_le_bytes());
    buf.extend_from_slice(&2835u32.to_le_bytes());
    buf.extend_from_slice(&2835u32.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    let padding = (row_size - w * 3) as usize;
    for y in 0..h {
        for x in 0..w {
            let r = ((x * 255) / w.max(1)) as u8;
            let g = ((y * 255) / h.max(1)) as u8;
            buf.extend_from_slice(&[100, g, r]);
        }
        buf.extend_from_slice(&vec![0u8; padding]);
    }
    buf
}

fn write_png_chunk(out: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(chunk_type);
    out.extend_from_slice(data);
    let mut hasher = Hasher::new();
    hasher.update(chunk_type);
    hasher.update(data);
    out.extend_from_slice(&hasher.finalize().to_be_bytes());
}

fn make_test_png(w: u32, h: u32) -> Vec<u8> {
    let mut buf = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    write_png_chunk(&mut buf, b"IHDR", &ihdr);

    let mut raw_scanlines = Vec::new();
    for y in 0..h {
        raw_scanlines.push(0u8); // Filter type None
        for x in 0..w {
            let r = ((x * 255) / w.max(1)) as u8;
            let g = ((y * 255) / h.max(1)) as u8;
            raw_scanlines.extend_from_slice(&[r, g, 200, 255]);
        }
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&raw_scanlines).unwrap();
    let compressed = encoder.finish().unwrap();
    write_png_chunk(&mut buf, b"IDAT", &compressed);
    write_png_chunk(&mut buf, b"IEND", &[]);
    buf
}


// 1x1 lossy VP8 valid WebP image (solid yellow)
const MINIMAL_WEBP_LOSSY: &[u8] = &[
    0x52, 0x49, 0x46, 0x46, 0x38, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50,
    0x56, 0x50, 0x38, 0x20, 0x2C, 0x00, 0x00, 0x00, 0xD0, 0x01, 0x00, 0x9D,
    0x01, 0x2A, 0x01, 0x00, 0x01, 0x00, 0x02, 0x00, 0x34, 0x25, 0xA4, 0x00,
    0x03, 0x70, 0x00, 0xFE, 0xFB, 0xFD, 0x50, 0x00, 0x00, 0x40, 0x01, 0x3C,
    0x9F, 0xFF, 0xFB, 0xFD, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
];

#[test]
fn test_aspect_dimensions() {
    assert_eq!(calculate_aspect_dimensions(3840, 2160, 256, 256), (256, 144));
    assert_eq!(calculate_aspect_dimensions(1000, 1000, 200, 100), (100, 100));
    assert_eq!(calculate_aspect_dimensions(1080, 1920, 300, 300), (169, 300));
    assert_eq!(calculate_aspect_dimensions(0, 0, 100, 100), (1, 1));
}

#[test]
fn test_decode_and_thumbnail_bmp() {
    let bmp_bytes = make_test_bmp(64, 48);
    let decoded = decode_image_rgba(&bmp_bytes).expect("Failed to decode BMP");
    assert_eq!(decoded.width, 64);
    assert_eq!(decoded.height, 48);
    assert_eq!(decoded.data.len(), 64 * 48 * 4);

    for filter in [ThumbnailFilter::Nearest, ThumbnailFilter::Bilinear, ThumbnailFilter::Lanczos3] {
        let thumb = generate_thumbnail(&bmp_bytes, 32, 32, filter)
            .expect("Thumbnail generation failed for BMP");
        assert_eq!(thumb.width, 32);
        assert_eq!(thumb.height, 24); // 64x48 scaled to 32x32 bounding box is 32x24
        assert_eq!(thumb.data.len(), (32 * 24 * 4) as usize);
    }
}

#[test]
fn test_decode_and_thumbnail_png() {
    let png_bytes = make_test_png(80, 60);
    let decoded = decode_image_rgba(&png_bytes).expect("Failed to decode PNG");
    assert_eq!(decoded.width, 80);
    assert_eq!(decoded.height, 60);
    assert_eq!(decoded.data.len(), 80 * 60 * 4);

    let thumb_lanczos = generate_thumbnail(&png_bytes, 40, 40, ThumbnailFilter::Lanczos3)
        .expect("Lanczos3 thumbnail generation failed for PNG");
    assert_eq!(thumb_lanczos.width, 40);
    assert_eq!(thumb_lanczos.height, 30);
    assert_eq!(thumb_lanczos.data.len(), (40 * 30 * 4) as usize);
}

fn make_test_jpeg(w: u32, h: u32) -> Vec<u8> {
    let mut raw = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let r = ((x * 255) / w.max(1)) as u8;
            let g = ((y * 255) / h.max(1)) as u8;
            raw.extend_from_slice(&[r, g, 150]);
        }
    }
    let mut out = Vec::new();
    let encoder = jpeg_encoder::Encoder::new(&mut out, 85);
    encoder.encode(&raw, w as u16, h as u16, jpeg_encoder::ColorType::Rgb).unwrap();
    out
}

#[test]
fn test_decode_and_thumbnail_jpeg() {
    let jpeg_bytes = make_test_jpeg(128, 96);
    let decoded = decode_image_rgba(&jpeg_bytes).expect("Failed to decode JPEG");
    assert_eq!(decoded.width, 128);
    assert_eq!(decoded.height, 96);
    assert_eq!(decoded.data.len(), 128 * 96 * 4);

    let thumb_lanczos = generate_thumbnail(&jpeg_bytes, 64, 64, ThumbnailFilter::Lanczos3)
        .expect("Lanczos3 thumbnail failed for JPEG");
    assert_eq!(thumb_lanczos.width, 64);
    assert_eq!(thumb_lanczos.height, 48);
    assert_eq!(thumb_lanczos.data.len(), (64 * 48 * 4) as usize);
}

#[test]
fn test_decode_webp() {
    let decoded = decode_image_rgba(MINIMAL_WEBP_LOSSY).expect("Failed to decode lossy WebP");
    assert_eq!(decoded.width, 1);
    assert_eq!(decoded.height, 1);
    assert_eq!(decoded.data.len(), 4);
    assert_eq!(decoded.data[3], 255); // Opaque alpha
}

#[test]
fn test_resize_algorithms_direct() {
    let src = DecodedImageRgba {
        width: 4,
        height: 4,
        data: vec![
            255, 0, 0, 255,   0, 255, 0, 255,   0, 0, 255, 255,   255, 255, 0, 255,
            0, 255, 255, 255, 255, 0, 255, 255, 128, 128, 128, 255, 64, 64, 64, 255,
            32, 32, 32, 255,  16, 16, 16, 255,  8, 8, 8, 255,     4, 4, 4, 255,
            2, 2, 2, 255,     1, 1, 1, 255,     0, 0, 0, 255,     255, 255, 255, 255,
        ],
    };

    let nearest = resize_rgba(&src, 2, 2, ThumbnailFilter::Nearest);
    assert_eq!(nearest.width, 2);
    assert_eq!(nearest.height, 2);
    assert_eq!(nearest.data.len(), 16);

    let bilinear = resize_rgba(&src, 2, 2, ThumbnailFilter::Bilinear);
    assert_eq!(bilinear.width, 2);
    assert_eq!(bilinear.height, 2);
    assert_eq!(bilinear.data.len(), 16);

    let lanczos = resize_rgba(&src, 2, 2, ThumbnailFilter::Lanczos3);
    assert_eq!(lanczos.width, 2);
    assert_eq!(lanczos.height, 2);
    assert_eq!(lanczos.data.len(), 16);
}

#[test]
fn test_uniffi_bindings_export() {
    let bmp = make_test_bmp(30, 20);
    let rec = decode_image_rgba_from_memory(bmp.clone()).expect("UniFFI decode failed");
    assert_eq!(rec.width, 30);
    assert_eq!(rec.height, 20);
    assert_eq!(rec.rgba_bytes.len(), 30 * 20 * 4);

    let thumb_rec = generate_thumbnail_from_memory(bmp, 15, 15, ThumbnailSamplingFilter::Lanczos3)
        .expect("UniFFI thumbnail failed");
    assert_eq!(thumb_rec.width, 15);
    assert_eq!(thumb_rec.height, 10);
    assert_eq!(thumb_rec.rgba_bytes.len(), 15 * 10 * 4);
}

#[test]
fn test_error_handling() {
    assert!(matches!(
        decode_image_rgba(&[]),
        Err(ImagePipelineError::EmptyData)
    ));
    assert!(matches!(
        decode_image_rgba(b"garbage_random_non_image_bytes"),
        Err(ImagePipelineError::DecodeFailed(_))
    ));
    assert!(matches!(
        generate_thumbnail(&make_test_bmp(10, 10), 0, 50, ThumbnailFilter::Lanczos3),
        Err(ImagePipelineError::InvalidDimensions(0, 50))
    ));
}
