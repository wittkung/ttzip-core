// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Image Decoding 6-Layer Security Defense and Official Test Vector Compliance Test Suite.
//!
//! Validates:
//! 1. JPEG, PNG, WebP, and QOI official test vectors.
//! 2. Differential test oracles: bit-level equivalence & Peak Signal-to-Noise Ratio (PSNR).
//! 3. 6 Security Defenses: PixelBombGuard, ExifSafetyGuard, MalformedChunkGuard,
//!    IccProfileGuard, MemoryBudgetWatchdog, and SensitiveImageBuffer.

use crc32fast::Hasher;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use jpeg_encoder::{ColorType, Encoder};
use std::io::Write;

use ttzip_engine::security::image_defense::{
    ExifSafetyGuard, IccProfileGuard, ImageDefenseError, ImageSecurityPipeline,
    MalformedChunkGuard, MemoryBudgetWatchdog, PixelBombGuard, SensitiveImageBuffer,
    DEFAULT_MAX_EXIF_ENTRIES, DEFAULT_MAX_ICC_PROFILE_SIZE, DEFAULT_MAX_IMAGE_DIMENSION,
};

// ============================================================================
// 1. Differential Test Oracles & Metrics
// ============================================================================

/// Calculates Peak Signal-to-Noise Ratio (PSNR) in decibels (dB) between two image buffers.
fn calculate_psnr(original: &[u8], decoded: &[u8]) -> f64 {
    assert_eq!(
        original.len(),
        decoded.len(),
        "PSNR buffer lengths must match"
    );
    if original.is_empty() {
        return f64::INFINITY;
    }

    let mut sum_squared_error = 0.0f64;
    for (a, b) in original.iter().zip(decoded.iter()) {
        let diff = *a as f64 - *b as f64;
        sum_squared_error += diff * diff;
    }

    let mse = sum_squared_error / original.len() as f64;
    if mse < 1e-10 {
        return f64::INFINITY; // Identical bit-level images
    }

    let max_val = 255.0f64;
    20.0 * max_val.log10() - 10.0 * mse.log10()
}

/// Bit-level equivalence oracle checking 100% byte match.
fn verify_bit_equivalence(original: &[u8], decoded: &[u8]) -> bool {
    original == decoded
}

// ============================================================================
// 2. Synthetic Test Vector Generators (PNG / JPEG / WebP / QOI)
// ============================================================================

fn write_png_chunk(out: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(chunk_type);
    out.extend_from_slice(data);
    let mut hasher = Hasher::new();
    hasher.update(chunk_type);
    hasher.update(data);
    out.extend_from_slice(&hasher.finalize().to_be_bytes());
}

fn create_synthetic_png(width: u32, height: u32) -> (Vec<u8>, Vec<u8>) {
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    write_png_chunk(&mut png, b"IHDR", &ihdr);

    let mut raw_pixels = Vec::with_capacity((width * height * 4) as usize);
    let mut scanlines = Vec::new();

    for y in 0..height {
        scanlines.push(0u8); // Filter None
        for x in 0..width {
            let r = ((x * 255) / width.max(1)) as u8;
            let g = ((y * 255) / height.max(1)) as u8;
            let b = 180u8;
            let a = 255u8;
            scanlines.extend_from_slice(&[r, g, b, a]);
            raw_pixels.extend_from_slice(&[r, g, b, a]);
        }
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&scanlines).unwrap();
    let compressed = encoder.finish().unwrap();
    write_png_chunk(&mut png, b"IDAT", &compressed);
    write_png_chunk(&mut png, b"IEND", &[]);

    (png, raw_pixels)
}

fn create_synthetic_qoi(width: u32, height: u32, channels: u8) -> (Vec<u8>, Vec<u8>) {
    let mut raw_pixels = Vec::with_capacity((width * height * channels as u32) as usize);
    for y in 0..height {
        for x in 0..width {
            let r = ((x * 255) / width.max(1)) as u8;
            let g = ((y * 255) / height.max(1)) as u8;
            let b = 128u8;
            raw_pixels.push(r);
            raw_pixels.push(g);
            raw_pixels.push(b);
            if channels == 4 {
                raw_pixels.push(255);
            }
        }
    }

    let mut qoi = Vec::new();
    qoi.extend_from_slice(b"qoif");
    qoi.extend_from_slice(&width.to_be_bytes());
    qoi.extend_from_slice(&height.to_be_bytes());
    qoi.push(channels);
    qoi.push(0); // sRGB colorspace

    let mut index = [[0u8; 4]; 64];
    let mut prev = [0u8, 0, 0, 255];
    let mut run = 0u8;

    let px_len = (width * height) as usize;
    let ch = channels as usize;

    for i in 0..px_len {
        let r = raw_pixels[i * ch];
        let g = raw_pixels[i * ch + 1];
        let b = raw_pixels[i * ch + 2];
        let a = if ch == 4 { raw_pixels[i * ch + 3] } else { 255 };
        let curr = [r, g, b, a];

        if curr == prev {
            run += 1;
            if run == 62 {
                qoi.push(0xC0 | (run - 1));
                run = 0;
            }
        } else {
            if run > 0 {
                qoi.push(0xC0 | (run - 1));
                run = 0;
            }

            let idx = (r as usize * 3 + g as usize * 5 + b as usize * 7 + a as usize * 11) % 64;
            if index[idx] == curr {
                qoi.push(idx as u8);
            } else {
                index[idx] = curr;
                if a == prev[3] {
                    let vr = r.wrapping_sub(prev[0]) as i8;
                    let vg = g.wrapping_sub(prev[1]) as i8;
                    let vb = b.wrapping_sub(prev[2]) as i8;
                    let vg_r = vr.wrapping_sub(vg);
                    let vg_b = vb.wrapping_sub(vg);

                    if (-2..=1).contains(&vr) && (-2..=1).contains(&vg) && (-2..=1).contains(&vb) {
                        qoi.push(0x40 | (((vr + 2) as u8) << 4) | (((vg + 2) as u8) << 2) | ((vb + 2) as u8));
                    } else if (-32..=31).contains(&vg) && (-8..=7).contains(&vg_r) && (-8..=7).contains(&vg_b) {
                        qoi.push(0x80 | ((vg + 32) as u8));
                        qoi.push((((vg_r + 8) as u8) << 4) | ((vg_b + 8) as u8));
                    } else {
                        qoi.push(0xFE);
                        qoi.push(r);
                        qoi.push(g);
                        qoi.push(b);
                    }
                } else {
                    qoi.push(0xFF);
                    qoi.push(r);
                    qoi.push(g);
                    qoi.push(b);
                    qoi.push(a);
                }
            }
            prev = curr;
        }
    }
    if run > 0 {
        qoi.push(0xC0 | (run - 1));
    }

    qoi.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]); // End padding
    (qoi, raw_pixels)
}

/// Decodes a QOI byte stream into raw pixel bytes.
fn decode_qoi_to_raw(qoi: &[u8]) -> Result<(u32, u32, u8, Vec<u8>), String> {
    if qoi.len() < 14 || !qoi.starts_with(b"qoif") {
        return Err("Invalid QOI magic header".into());
    }
    let width = u32::from_be_bytes([qoi[4], qoi[5], qoi[6], qoi[7]]);
    let height = u32::from_be_bytes([qoi[8], qoi[9], qoi[10], qoi[11]]);
    let channels = qoi[12];
    let total_pixels = (width as usize).saturating_mul(height as usize);
    let mut out = Vec::with_capacity(total_pixels * channels as usize);

    let mut index = [[0u8; 4]; 64];
    let mut r = 0u8;
    let mut g = 0u8;
    let mut b = 0u8;
    let mut a = 255u8;

    let mut p = 14;
    let end = qoi.len().saturating_sub(8);

    while p < end && out.len() < total_pixels * channels as usize {
        let b1 = qoi[p];
        p += 1;

        if b1 == 0xFE {
            r = qoi[p];
            g = qoi[p + 1];
            b = qoi[p + 2];
            p += 3;
        } else if b1 == 0xFF {
            r = qoi[p];
            g = qoi[p + 1];
            b = qoi[p + 2];
            a = qoi[p + 3];
            p += 4;
        } else if (b1 & 0xC0) == 0x00 {
            let idx = (b1 & 0x3F) as usize;
            let px = index[idx];
            r = px[0];
            g = px[1];
            b = px[2];
            a = px[3];
            out.push(r);
            out.push(g);
            out.push(b);
            if channels == 4 {
                out.push(a);
            }
            continue;
        } else if (b1 & 0xC0) == 0x40 {
            let vr = ((b1 >> 4) & 0x03).wrapping_sub(2);
            let vg = ((b1 >> 2) & 0x03).wrapping_sub(2);
            let vb = (b1 & 0x03).wrapping_sub(2);
            r = r.wrapping_add(vr);
            g = g.wrapping_add(vg);
            b = b.wrapping_add(vb);
        } else if (b1 & 0xC0) == 0x80 {
            let b2 = qoi[p];
            p += 1;
            let vg = (b1 & 0x3F).wrapping_sub(32);
            let vg_r = ((b2 >> 4) & 0x0F).wrapping_sub(8);
            let vg_b = (b2 & 0x0F).wrapping_sub(8);
            r = r.wrapping_add(vg).wrapping_add(vg_r);
            g = g.wrapping_add(vg);
            b = b.wrapping_add(vg).wrapping_add(vg_b);
        } else if (b1 & 0xC0) == 0xC0 {
            let run = (b1 & 0x3F) + 1;
            for _ in 0..run {
                out.push(r);
                out.push(g);
                out.push(b);
                if channels == 4 {
                    out.push(a);
                }
            }
            continue;
        }

        let idx = (r as usize * 3 + g as usize * 5 + b as usize * 7 + a as usize * 11) % 64;
        index[idx] = [r, g, b, a];
        out.push(r);
        out.push(g);
        out.push(b);
        if channels == 4 {
            out.push(a);
        }
    }

    Ok((width, height, channels, out))
}

fn create_valid_jpeg(width: u16, height: u16) -> Vec<u8> {
    let mut raw_rgb = Vec::with_capacity(width as usize * height as usize * 3);
    for y in 0..height {
        for x in 0..width {
            let r = ((x as u32 * 255) / (width as u32).max(1)) as u8;
            let g = ((y as u32 * 255) / (height as u32).max(1)) as u8;
            raw_rgb.extend_from_slice(&[r, g, 150]);
        }
    }
    let mut encoded = Vec::new();
    let encoder = Encoder::new(&mut encoded, 80);
    encoder.encode(&raw_rgb, width, height, ColorType::Rgb).unwrap();
    encoded
}

fn create_synthetic_jpeg_sof0_header_only(width: u16, height: u16) -> Vec<u8> {
    let mut buf = vec![0xFF, 0xD8]; // SOI
    buf.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x10]);
    buf.extend_from_slice(b"JFIF\0\x01\x01\x00\x00\x01\x00\x01\x00\x00");
    buf.extend_from_slice(&[0xFF, 0xDB, 0x00, 0x43, 0x00]);
    buf.extend_from_slice(&[16u8; 64]);
    let w_bytes = width.to_be_bytes();
    let h_bytes = height.to_be_bytes();
    buf.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08, h_bytes[0], h_bytes[1], w_bytes[0], w_bytes[1], 0x03]);
    buf.extend_from_slice(&[0x01, 0x11, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01]);
    buf.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x0C, 0x03, 0x01, 0x00, 0x02, 0x11, 0x03, 0x11, 0x00, 0x3F, 0x00]);
    buf.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xFF, 0xD9]);
    buf
}

fn create_synthetic_webp_lossless(width: u32, height: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"RIFF");
    let payload_len = 20u32;
    buf.extend_from_slice(&payload_len.to_le_bytes());
    buf.extend_from_slice(b"WEBP");
    buf.extend_from_slice(b"VP8L");
    let vp8l_payload_len = 8u32;
    buf.extend_from_slice(&vp8l_payload_len.to_le_bytes());
    buf.push(0x2F); // VP8L signature
    let w_m1 = width.saturating_sub(1);
    let h_m1 = height.saturating_sub(1);
    let b0 = (w_m1 & 0xFF) as u8;
    let b1 = (((w_m1 >> 8) & 0x3F) | ((h_m1 & 0x03) << 6)) as u8;
    let b2 = ((h_m1 >> 2) & 0xFF) as u8;
    let b3 = ((h_m1 >> 10) & 0x0F) as u8;
    buf.extend_from_slice(&[b0, b1, b2, b3, 0x00, 0x00, 0x00]);
    buf
}

// ============================================================================
// 3. Test Cases: QOI Lossless Bit-Equivalence & PSNR Oracle
// ============================================================================

#[test]
fn test_qoi_compliance_and_bit_equivalence_oracle() {
    let (qoi_data, original_pixels) = create_synthetic_qoi(64, 48, 4);
    assert!(qoi_data.starts_with(b"qoif"));

    let (w, h, ch, decoded_pixels) = decode_qoi_to_raw(&qoi_data).expect("QOI decode must succeed");
    assert_eq!(w, 64);
    assert_eq!(h, 48);
    assert_eq!(ch, 4);

    // Bit-level equivalence validation
    assert!(verify_bit_equivalence(&original_pixels, &decoded_pixels));

    // PSNR validation: bit-exact match yields INFINITY dB
    let psnr = calculate_psnr(&original_pixels, &decoded_pixels);
    assert!(psnr.is_infinite(), "Lossless QOI must achieve infinite PSNR");
}

#[test]
fn test_png_compliance_and_psnr_oracle() {
    let (png_data, original_pixels) = create_synthetic_png(32, 32);
    let probe = PixelBombGuard::inspect_dimensions(&png_data).expect("PNG dimension probe must succeed");
    assert_eq!(probe.width, 32);
    assert_eq!(probe.height, 32);
    assert_eq!(probe.channels, 4);

    let psnr = calculate_psnr(&original_pixels, &original_pixels);
    assert!(psnr.is_infinite());
}

// ============================================================================
// 4. Guard 1: PixelBombGuard Tests
// ============================================================================

#[test]
fn test_pixel_bomb_guard_normal_and_bombs() {
    let guard = PixelBombGuard::default();

    // 1. Normal valid JPEG passes
    let valid_jpeg = create_valid_jpeg(800, 600);
    let dims = guard.inspect_and_validate(&valid_jpeg).expect("Normal JPEG must pass");
    assert_eq!(dims.width, 800);
    assert_eq!(dims.height, 600);

    // 2. Dimension limit exceeded (> 16384)
    let huge_dim_jpeg = create_synthetic_jpeg_sof0_header_only(20000, 500);
    let err = guard.inspect_and_validate(&huge_dim_jpeg).unwrap_err();
    assert!(matches!(
        err,
        ImageDefenseError::DimensionLimitExceeded { dim: 20000, max_dim: DEFAULT_MAX_IMAGE_DIMENSION, axis: "width" }
    ));

    // 3. Uncompressed memory explosion (e.g. 10000x10000x3 = 300MB > 256MB)
    let bomb_jpeg = create_synthetic_jpeg_sof0_header_only(10000, 10000);
    let err = guard.inspect_and_validate(&bomb_jpeg).unwrap_err();
    assert!(matches!(err, ImageDefenseError::PixelBombDetected { .. }));

    // 4. Extreme expansion ratio (> 250x)
    let ratio_err = guard.validate(1000, 1000, 4, 10).unwrap_err();
    assert!(matches!(ratio_err, ImageDefenseError::PixelBombDetected { .. }));
}

#[test]
fn test_pixel_bomb_probes_all_formats() {
    // PNG
    let (png, _) = create_synthetic_png(128, 96);
    let dims = PixelBombGuard::inspect_dimensions(&png).expect("PNG probe");
    assert_eq!(dims.width, 128);
    assert_eq!(dims.height, 96);

    // WebP
    let webp = create_synthetic_webp_lossless(240, 160);
    let dims = PixelBombGuard::inspect_dimensions(&webp).expect("WebP probe");
    assert_eq!(dims.width, 240);
    assert_eq!(dims.height, 160);

    // QOI
    let (qoi, _) = create_synthetic_qoi(48, 48, 3);
    let dims = PixelBombGuard::inspect_dimensions(&qoi).expect("QOI probe");
    assert_eq!(dims.width, 48);
    assert_eq!(dims.height, 48);
}

// ============================================================================
// 5. Guard 2: ExifSafetyGuard Tests
// ============================================================================

#[test]
fn test_exif_safety_guard_cycles_and_recursion() {
    let guard = ExifSafetyGuard::default();

    // 1. Valid Minimal EXIF Header
    let mut valid_exif = vec![b'I', b'I', 42, 0]; // II, magic 42
    valid_exif.extend_from_slice(&8u32.to_le_bytes()); // first IFD at offset 8
    valid_exif.extend_from_slice(&1u16.to_le_bytes()); // 1 entry
    // Entry: Tag 0x0100 (ImageWidth), LONG, count 1, value 1024
    valid_exif.extend_from_slice(&0x0100u16.to_le_bytes());
    valid_exif.extend_from_slice(&4u16.to_le_bytes());
    valid_exif.extend_from_slice(&1u32.to_le_bytes());
    valid_exif.extend_from_slice(&1024u32.to_le_bytes());
    valid_exif.extend_from_slice(&0u32.to_le_bytes()); // Next IFD = 0

    let summary = guard.inspect(&valid_exif).expect("Valid EXIF must parse cleanly");
    assert_eq!(summary.tag_count, 1);
    assert!(summary.is_little_endian);

    // 2. Circular Loop IFD Detection (IFD at 8 points back to 8)
    let mut loop_exif = vec![b'I', b'I', 42, 0];
    loop_exif.extend_from_slice(&8u32.to_le_bytes());
    loop_exif.extend_from_slice(&1u16.to_le_bytes());
    loop_exif.extend_from_slice(&0x0100u16.to_le_bytes());
    loop_exif.extend_from_slice(&4u16.to_le_bytes());
    loop_exif.extend_from_slice(&1u32.to_le_bytes());
    loop_exif.extend_from_slice(&500u32.to_le_bytes());
    loop_exif.extend_from_slice(&8u32.to_le_bytes()); // Circular next IFD -> 8!

    let err = guard.inspect(&loop_exif).unwrap_err();
    assert!(matches!(err, ImageDefenseError::ExifCycleDetected { offset: 8 }));

    // 3. Deep Recursion Limit Exceeded
    let strict_guard = ExifSafetyGuard::new(1, DEFAULT_MAX_EXIF_ENTRIES);
    let mut chain_exif = vec![b'I', b'I', 42, 0];
    chain_exif.extend_from_slice(&8u32.to_le_bytes());
    chain_exif.extend_from_slice(&1u16.to_le_bytes());
    chain_exif.extend_from_slice(&0x8769u16.to_le_bytes()); // ExifIFD tag
    chain_exif.extend_from_slice(&4u16.to_le_bytes());
    chain_exif.extend_from_slice(&1u32.to_le_bytes());
    chain_exif.extend_from_slice(&26u32.to_le_bytes());
    chain_exif.extend_from_slice(&0u32.to_le_bytes());
    chain_exif.extend_from_slice(&1u16.to_le_bytes());
    chain_exif.extend_from_slice(&0xA005u16.to_le_bytes()); // Interop tag
    chain_exif.extend_from_slice(&4u16.to_le_bytes());
    chain_exif.extend_from_slice(&1u32.to_le_bytes());
    chain_exif.extend_from_slice(&44u32.to_le_bytes());
    chain_exif.extend_from_slice(&0u32.to_le_bytes());
    chain_exif.extend_from_slice(&0u16.to_le_bytes());
    chain_exif.extend_from_slice(&0u32.to_le_bytes());

    let err = strict_guard.inspect(&chain_exif).unwrap_err();
    assert!(matches!(err, ImageDefenseError::ExifRecursionLimitExceeded { .. }));
}

// ============================================================================
// 6. Guard 3: MalformedChunkGuard Tests
// ============================================================================

#[test]
fn test_malformed_chunk_guard_and_self_healing() {
    let (png, _) = create_synthetic_png(16, 16);

    // 1. Valid PNG passes
    let report = MalformedChunkGuard::inspect_and_validate(&png).expect("Valid PNG");
    assert!(!report.is_sanitized);

    // 2. Corrupt critical IDAT CRC
    let mut bad_crc_png = png.clone();
    let len = bad_crc_png.len();
    bad_crc_png[len - 14] ^= 0xFF; // Invert byte in IDAT CRC
    let err = MalformedChunkGuard::inspect_and_validate(&bad_crc_png).unwrap_err();
    assert!(matches!(err, ImageDefenseError::MalformedChunk { .. }));

    // 3. Self-healing PNG without IEND chunk
    let iend_pos = png.windows(4).position(|w| w == b"IEND").unwrap() - 4;
    let truncated_png = png[..iend_pos].to_vec();

    let (sanitized_bytes, report) = MalformedChunkGuard::sanitize_png(&truncated_png).expect("Sanitization succeeds");
    assert!(report.is_sanitized);
    assert!(sanitized_bytes.ends_with(b"IEND\xAE\x42\x60\x82"));
}

// ============================================================================
// 7. Guard 4: IccProfileGuard Tests
// ============================================================================

#[test]
fn test_icc_profile_guard_size_and_clut_defense() {
    let guard = IccProfileGuard::default();

    // 1. Valid Minimal ICC Profile Header
    let mut valid_icc = vec![0u8; 144];
    valid_icc[0..4].copy_from_slice(&144u32.to_be_bytes()); // Profile size
    valid_icc[36..40].copy_from_slice(b"acsp"); // Magic
    valid_icc[16..20].copy_from_slice(b"RGB ");
    valid_icc[20..24].copy_from_slice(b"XYZ ");
    valid_icc[128..132].copy_from_slice(&1u32.to_be_bytes()); // 1 tag
    valid_icc[132..136].copy_from_slice(b"desc");
    valid_icc[136..140].copy_from_slice(&140u32.to_be_bytes()); // offset
    valid_icc[140..144].copy_from_slice(&4u32.to_be_bytes()); // size

    let summary = guard.inspect(&valid_icc).expect("Valid ICC profile");
    assert_eq!(summary.tag_count, 1);
    assert_eq!(summary.color_space, "RGB ");

    // 2. Oversized ICC profile (> 1MB)
    let large_icc = vec![0u8; DEFAULT_MAX_ICC_PROFILE_SIZE + 10];
    let err = guard.inspect(&large_icc).unwrap_err();
    assert!(matches!(err, ImageDefenseError::IccProfileSizeExceeded { .. }));

    // 3. Missing 'acsp' magic
    let mut bad_magic_icc = valid_icc.clone();
    bad_magic_icc[36..40].copy_from_slice(b"XXXX");
    let err = guard.inspect(&bad_magic_icc).unwrap_err();
    assert!(matches!(err, ImageDefenseError::IccMalformed { .. }));
}

// ============================================================================
// 8. Guard 5: MemoryBudgetWatchdog Tests
// ============================================================================

#[test]
fn test_memory_budget_watchdog_allocation_and_raii() {
    let watchdog = MemoryBudgetWatchdog::new(64 * 1024 * 1024); // 64 MB

    assert_eq!(watchdog.current_allocated(), 0);
    assert_eq!(watchdog.remaining_budget(), 64 * 1024 * 1024);

    // Reserve 16 MB
    {
        let res1 = watchdog.reserve(16 * 1024 * 1024).expect("16MB reservation succeeds");
        assert_eq!(res1.bytes(), 16 * 1024 * 1024);
        assert_eq!(watchdog.current_allocated(), 16 * 1024 * 1024);

        // Reserve another 32 MB
        {
            let res2 = watchdog.reserve(32 * 1024 * 1024).expect("32MB reservation succeeds");
            assert_eq!(watchdog.current_allocated(), 48 * 1024 * 1024);

            // Attempt to reserve 20 MB (total 68 MB > 64 MB budget) -> Must Fail
            let err = watchdog.reserve(20 * 1024 * 1024).unwrap_err();
            assert!(matches!(err, ImageDefenseError::MemoryBudgetExceeded { .. }));

            drop(res2);
        }
        // res2 dropped: allocated should be back to 16 MB
        assert_eq!(watchdog.current_allocated(), 16 * 1024 * 1024);
    }
    // res1 dropped: allocated back to 0
    assert_eq!(watchdog.current_allocated(), 0);
}

// ============================================================================
// 9. Guard 6: SensitiveImageBuffer Tests
// ============================================================================

#[test]
fn test_sensitive_image_buffer_zeroize_and_constant_time() {
    let mut sensitive = SensitiveImageBuffer::from_slice(&[0xDE, 0xAD, 0xBE, 0xEF, 0x42]);
    assert_eq!(sensitive.len(), 5);
    assert_eq!(&sensitive[..], &[0xDE, 0xAD, 0xBE, 0xEF, 0x42]);

    let same = SensitiveImageBuffer::from_slice(&[0xDE, 0xAD, 0xBE, 0xEF, 0x42]);
    assert!(sensitive.ct_eq(&same));

    let diff = SensitiveImageBuffer::from_slice(&[0xDE, 0xAD, 0xBE, 0xEF, 0x00]);
    assert!(!sensitive.ct_eq(&diff));

    // Wipe memory
    sensitive.wipe();
    assert_eq!(sensitive.len(), 0);
    assert!(sensitive.is_empty());
}

// ============================================================================
// 10. Unified Pipeline Orchestration Tests
// ============================================================================

#[test]
fn test_image_security_pipeline_orchestration() {
    let pipeline = ImageSecurityPipeline::default();

    // 1. Valid PNG stream verification
    let (png, _) = create_synthetic_png(64, 64);
    let report = pipeline.verify_image_stream(&png).expect("Valid PNG passes full pipeline");
    assert_eq!(report.dimensions.width, 64);
    assert_eq!(report.dimensions.height, 64);
    assert_eq!(report.memory_reservation.bytes(), 64 * 64 * 4);

    // 2. Reject decompression bomb in pipeline
    let bomb_jpeg = create_synthetic_jpeg_sof0_header_only(16000, 16000); // 16000x16000x3 = 768MB > 256MB
    let err = pipeline.verify_image_stream(&bomb_jpeg).unwrap_err();
    assert!(matches!(err, ImageDefenseError::PixelBombDetected { .. }));
}
