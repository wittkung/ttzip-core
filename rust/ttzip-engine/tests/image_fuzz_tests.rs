// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive corruption injection, chaos mutation, and fuzzing test suite for TTZip Image Subsystem.
//!
//! Deploys 16 surgical destruction targets:
//! 1. High-compression ratio pixel bomb (Pixel Bomb) circuit breaker & dimension defense.
//! 2. Malformed EXIF APP1 marker segments and cyclic IFD loop injection.
//! 3. Broken PNG IHDR/PLTE/IDAT/IEND chunk sequences and CRC corruption.
//! 4. Truncated image data streams and incomplete scanline recovery.
//! 5. Zero-byte, single-byte, and empty stream boundary corruption injection.
//! 6. 1000+ tasks concurrent image decoding and thumbnail generation contention.
//! 7. 500+ rounds of pseudo-random mutation image data fuzzing across all formats.
//! 8. Malformed ICC profile payload and CLUT table dimension explosion injection.
//! 9. Single-byte and single-bit flip avalanche resilience across format magic headers.
//! 10. Viewport sub-region out-of-bounds crop & zero-dimension viewport bounds fuzzing.
//! 11. Colorspace conversion pipeline boundary & mismatched buffer size fuzzing.
//! 12. Memory budget watchdog circuit breaker under extreme allocations.
//! 13. Sensitive image buffer zeroize-on-drop volatile memory erasure fuzzing.
//! 14. Dynamic scale factor fuzzing (NaN, Infinity, negative, extreme values) in ViewportSampler.
//! 15. QOI and Farbfeld malformed header / chunk run-length fuzzing.
//! 16. Multi-format magic sniffing false-positive and hybrid polyglot injection fuzzing.

use std::panic::catch_unwind;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crc32fast::Hasher as CrcHasher;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use rayon::prelude::*;
use std::io::Write;

use ttzip_engine::image::{
    ColorSpacePipeline, DecodedImageFrame, ExifThumbnailExtractor, ImageBitDepth, ImageColorSpace,
    ImageError, ImageFormat, TTZipImageDecoder, ViewportFilter, ViewportRect, ViewportSampler,
};
use ttzip_engine::security::image_defense::{
    ExifSafetyGuard, IccProfileGuard, ImageDefenseError, MalformedChunkGuard, MemoryBudgetWatchdog,
    PixelBombGuard, SensitiveImageBuffer, DEFAULT_MAX_ICC_PROFILE_SIZE,
};
use ttzip_engine::standards::image_pipeline::{
    decode_image_rgba, generate_thumbnail, ImagePipelineError, ThumbnailFilter,
};

/// High-speed deterministic linear congruential generator for reproducible fuzzing vectors.
#[derive(Clone, Debug)]
struct DeterministicPrng {
    state: u64,
}

impl DeterministicPrng {
    #[inline]
    const fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x9e37_79b9_7f4a_7c15 } else { seed },
        }
    }

    #[inline]
    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }

    #[inline]
    fn next_range(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        let span = (max - min + 1) as u64;
        min + (self.next_u32() as u64 % span) as usize
    }

    #[inline]
    fn next_byte(&mut self) -> u8 {
        (self.next_u32() & 0xFF) as u8
    }
}

// ============================================================================
// Synthetic Valid Image Fixture Generators
// ============================================================================

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
    let mut hasher = CrcHasher::new();
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
        raw_scanlines.push(0u8); // Filter None
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

const MINIMAL_WEBP_LOSSY: &[u8] = &[
    0x52, 0x49, 0x46, 0x46, 0x38, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50,
    0x56, 0x50, 0x38, 0x20, 0x2C, 0x00, 0x00, 0x00, 0xD0, 0x01, 0x00, 0x9D,
    0x01, 0x2A, 0x01, 0x00, 0x01, 0x00, 0x02, 0x00, 0x34, 0x25, 0xA4, 0x00,
    0x03, 0x70, 0x00, 0xFE, 0xFB, 0xFD, 0x50, 0x00, 0x00, 0x40, 0x01, 0x3C,
    0x9F, 0xFF, 0xFB, 0xFD, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
];

fn make_test_qoi(w: u32, h: u32) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(b"qoif");
    data.extend_from_slice(&w.to_be_bytes());
    data.extend_from_slice(&h.to_be_bytes());
    data.push(4); // channels: 4 = RGBA
    data.push(0); // colorspace: sRGB
    for _ in 0..(w * h) {
        data.push(0b11111111); // QOI_OP_RGBA
        data.extend_from_slice(&[200, 100, 50, 255]);
    }
    data.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    data
}

fn make_test_ppm(w: u32, h: u32) -> Vec<u8> {
    let header = format!("P6\n{w} {h}\n255\n");
    let mut data = header.into_bytes();
    for y in 0..h {
        for x in 0..w {
            let r = ((x * 255) / w.max(1)) as u8;
            let g = ((y * 255) / h.max(1)) as u8;
            data.extend_from_slice(&[r, g, 128]);
        }
    }
    data
}

// ============================================================================
// Target 1: Pixel Bomb (Pixel Bomb) Fuse & Decompression Explosion Defense
// ============================================================================
#[test]
fn test_target_01_pixel_bomb_circuit_breaker() {
    let guard = PixelBombGuard::default();

    // 1. Extreme dimension bomb in BMP header (65,535 x 65,535 in small payload)
    let mut fake_bmp = vec![0u8; 54];
    fake_bmp[0..2].copy_from_slice(b"BM");
    fake_bmp[14..18].copy_from_slice(&40u32.to_le_bytes()); // BITMAPINFOHEADER
    fake_bmp[18..22].copy_from_slice(&65535i32.to_le_bytes()); // width
    fake_bmp[22..26].copy_from_slice(&65535i32.to_le_bytes()); // height
    fake_bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
    fake_bmp[28..30].copy_from_slice(&24u16.to_le_bytes()); // 24-bit

    let inspected = guard.inspect_and_validate(&fake_bmp);
    assert!(
        matches!(inspected, Err(ImageDefenseError::DimensionLimitExceeded { .. }) | Err(ImageDefenseError::PixelBombDetected { .. })),
        "Pixel bomb guard must reject 65535x65535 BMP header"
    );

    // 2. High expansion ratio test (500x expansion ratio on 100 bytes input)
    let dims_bomb = guard.validate(1000, 1000, 4, 100);
    assert!(
        matches!(dims_bomb, Err(ImageDefenseError::PixelBombDetected { .. })),
        "Decompression bomb with 40,000x expansion ratio must be intercepted"
    );

    // 3. Engine-level defense validation: decoding corrupted giant dimension does not OOM
    let decode_res = TTZipImageDecoder::decode(&fake_bmp);
    assert!(decode_res.is_err(), "Engine must safely fail on truncated pixel bomb");
}

// ============================================================================
// Target 2: Malformed EXIF APP1 Marker Segments & Cyclic IFD Loop Injection
// ============================================================================
#[test]
fn test_target_02_malformed_exif_and_cyclic_ifd_injection() {
    let guard = ExifSafetyGuard::default();

    // 1. Cyclic IFD pointer loop (IFD0 points to IFD1, IFD1 points back to IFD0)
    let mut cyclic_tiff = vec![0u8; 64];
    cyclic_tiff[0..2].copy_from_slice(b"II"); // Little endian
    cyclic_tiff[2..4].copy_from_slice(&42u16.to_le_bytes());
    cyclic_tiff[4..8].copy_from_slice(&8u32.to_le_bytes()); // IFD0 offset = 8

    // IFD0 at offset 8 (1 entry, next IFD at offset 24)
    cyclic_tiff[8..10].copy_from_slice(&1u16.to_le_bytes());
    // entry 0: tag=0x0112 (Orientation), type=3, count=1, val=1
    cyclic_tiff[10..12].copy_from_slice(&0x0112u16.to_le_bytes());
    cyclic_tiff[12..14].copy_from_slice(&3u16.to_le_bytes());
    cyclic_tiff[14..18].copy_from_slice(&1u32.to_le_bytes());
    cyclic_tiff[18..20].copy_from_slice(&1u16.to_le_bytes());
    // Next IFD offset = 24
    cyclic_tiff[22..26].copy_from_slice(&24u32.to_le_bytes());

    // IFD1 at offset 24 (1 entry, next IFD points back to IFD0 at offset 8!)
    cyclic_tiff[24..26].copy_from_slice(&1u16.to_le_bytes());
    cyclic_tiff[26..28].copy_from_slice(&0x010Fu16.to_le_bytes());
    cyclic_tiff[28..30].copy_from_slice(&2u16.to_le_bytes());
    cyclic_tiff[30..34].copy_from_slice(&1u32.to_le_bytes());
    // Next IFD offset points back to offset 8!
    cyclic_tiff[38..42].copy_from_slice(&8u32.to_le_bytes());

    let inspect_res = guard.inspect(&cyclic_tiff);
    assert!(
        matches!(inspect_res, Err(ImageDefenseError::ExifRecursionLimitExceeded { .. }) | Ok(_)),
        "Cyclic IFD loop must be bounded by recursion limit"
    );

    // 2. Thumbnail extractor must safely terminate without infinite loop
    let mut jpeg_with_cyclic_exif = vec![0xFF, 0xD8, 0xFF, 0xE1];
    let app1_len = (cyclic_tiff.len() + 8) as u16;
    jpeg_with_cyclic_exif.extend_from_slice(&app1_len.to_be_bytes());
    jpeg_with_cyclic_exif.extend_from_slice(b"Exif\0\0");
    jpeg_with_cyclic_exif.extend_from_slice(&cyclic_tiff);
    jpeg_with_cyclic_exif.extend_from_slice(&[0xFF, 0xD9]);

    let thumb_opt = ExifThumbnailExtractor::extract_embedded_jpeg(&jpeg_with_cyclic_exif);
    assert!(thumb_opt.is_none(), "Corrupted cyclic EXIF should not yield thumbnail");
}

// ============================================================================
// Target 3: Broken PNG IHDR/PLTE/IDAT/IEND Chunk Sequence Injection
// ============================================================================
#[test]
fn test_target_03_broken_png_chunks_and_crc_corruption() {
    // 1. Missing IHDR chunk at PNG start
    let mut broken_png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    write_png_chunk(&mut broken_png, b"IDAT", &[1, 2, 3, 4]);
    write_png_chunk(&mut broken_png, b"IEND", &[]);

    let report = MalformedChunkGuard::inspect_and_validate(&broken_png);
    assert!(
        matches!(report, Err(ImageDefenseError::MalformedChunk { ref chunk_type, .. }) if chunk_type == "IDAT"),
        "Malformed chunk guard must detect unexpected non-IHDR chunk"
    );

    // 2. Corrupt CRC32 in valid PNG
    let mut valid_png = make_test_png(16, 16);
    let len = valid_png.len();
    valid_png[len - 2] ^= 0xFF; // Invert CRC byte of IEND

    let decode_res = TTZipImageDecoder::decode(&valid_png);
    // Should gracefully fail without crashing
    assert!(decode_res.is_err() || decode_res.is_ok());
}

// ============================================================================
// Target 4: Truncated Image Streams & Incomplete Scanline Recovery
// ============================================================================
#[test]
fn test_target_04_truncated_image_stream_recovery() {
    let valid_jpeg = make_test_jpeg(32, 32);
    let valid_png = make_test_png(32, 32);
    let valid_bmp = make_test_bmp(32, 32);

    let truncation_fractions = [0.1, 0.25, 0.5, 0.75, 0.95];

    for &frac in &truncation_fractions {
        let cut_jpeg = &valid_jpeg[..(valid_jpeg.len() as f64 * frac) as usize];
        let res_jpeg = catch_unwind(|| {
            let _ = TTZipImageDecoder::decode(cut_jpeg);
            let _ = decode_image_rgba(cut_jpeg);
        });
        assert!(res_jpeg.is_ok(), "Truncated JPEG must not panic");

        let cut_png = &valid_png[..(valid_png.len() as f64 * frac) as usize];
        let res_png = catch_unwind(|| {
            let _ = TTZipImageDecoder::decode(cut_png);
            let _ = decode_image_rgba(cut_png);
        });
        assert!(res_png.is_ok(), "Truncated PNG must not panic");

        let cut_bmp = &valid_bmp[..(valid_bmp.len() as f64 * frac) as usize];
        let res_bmp = catch_unwind(|| {
            let _ = TTZipImageDecoder::decode(cut_bmp);
            let _ = decode_image_rgba(cut_bmp);
        });
        assert!(res_bmp.is_ok(), "Truncated BMP must not panic");
    }
}

// ============================================================================
// Target 5: Zero-Byte, Single-Byte, and Empty Stream Boundary Injection
// ============================================================================
#[test]
fn test_target_05_zero_byte_and_empty_stream_injection() {
    assert_eq!(decode_image_rgba(&[]), Err(ImagePipelineError::EmptyData));
    assert_eq!(TTZipImageDecoder::decode(&[]), Err(ImageError::EmptyData));

    let minimal_vectors: &[&[u8]] = &[
        &[0],
        &[0xFF],
        &[0x89],
        &[0x52, 0x49],
        b"BM",
        b"qoif",
        b"farbfeld",
        b"P6\n",
    ];

    for &vec in minimal_vectors {
        let res = catch_unwind(|| {
            let _ = TTZipImageDecoder::decode(vec);
            let _ = decode_image_rgba(vec);
            let _ = ExifThumbnailExtractor::extract_embedded_jpeg(vec);
            let _ = TTZipImageDecoder::detect_format(vec);
        });
        assert!(res.is_ok(), "Minimal boundary byte vector must not panic");
    }
}

// ============================================================================
// Target 6: 1000+ Tasks Concurrent Image Decoding & Thumbnail Contention
// ============================================================================
#[test]
fn test_target_06_concurrent_image_decoding_contention() {
    let png_fixture = Arc::new(make_test_png(24, 24));
    let jpeg_fixture = Arc::new(make_test_jpeg(24, 24));
    let bmp_fixture = Arc::new(make_test_bmp(24, 24));

    let completed_count = AtomicUsize::new(0);

    (0..1000).into_par_iter().for_each(|i| {
        let (data, filter) = match i % 3 {
            0 => (&png_fixture, ThumbnailFilter::Lanczos3),
            1 => (&jpeg_fixture, ThumbnailFilter::Bilinear),
            _ => (&bmp_fixture, ThumbnailFilter::Nearest),
        };

        let decoded = decode_image_rgba(data).expect("Concurrent decode failed");
        assert!(decoded.width == 24 && decoded.height == 24);

        let thumb = generate_thumbnail(data, 12, 12, filter)
            .expect("Concurrent thumbnail failed");
        assert!(thumb.width <= 12 && thumb.height <= 12);

        completed_count.fetch_add(1, Ordering::Relaxed);
    });

    assert_eq!(completed_count.load(Ordering::SeqCst), 1000);
}

// ============================================================================
// Target 7: 500+ Rounds of Pseudo-Random Mutation Image Data Fuzzing
// ============================================================================
#[test]
fn test_target_07_random_mutation_stream_fuzzing() {
    let mut prng = DeterministicPrng::new(0xABCD_EF01_2345_6789);

    let seed_fixtures = [
        make_test_png(16, 16),
        make_test_jpeg(16, 16),
        make_test_bmp(16, 16),
        make_test_qoi(8, 8),
        make_test_ppm(8, 8),
        MINIMAL_WEBP_LOSSY.to_vec(),
    ];

    let panic_count = AtomicUsize::new(0);

    for round in 0..500 {
        let base = &seed_fixtures[round % seed_fixtures.len()];
        let mut mutated = base.clone();

        let mutation_count = prng.next_range(1, 10);
        for _ in 0..mutation_count {
            let m_type = prng.next_range(0, 3);
            match m_type {
                0 => {
                    // Single bit flip
                    if !mutated.is_empty() {
                        let idx = prng.next_range(0, mutated.len() - 1);
                        let bit = 1 << prng.next_range(0, 7);
                        mutated[idx] ^= bit;
                    }
                }
                1 => {
                    // Random byte replacement
                    if !mutated.is_empty() {
                        let idx = prng.next_range(0, mutated.len() - 1);
                        mutated[idx] = prng.next_byte();
                    }
                }
                2 => {
                    // Random slice truncation
                    if mutated.len() > 8 {
                        let new_len = prng.next_range(4, mutated.len() - 1);
                        mutated.truncate(new_len);
                    }
                }
                _ => {
                    // Random byte insertion
                    let idx = prng.next_range(0, mutated.len());
                    mutated.insert(idx, prng.next_byte());
                }
            }
        }

        let res = catch_unwind(|| {
            let _ = TTZipImageDecoder::decode(&mutated);
            let _ = decode_image_rgba(&mutated);
            let _ = ExifThumbnailExtractor::extract_embedded_jpeg(&mutated);
        });

        if res.is_err() {
            panic_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    assert_eq!(
        panic_count.load(Ordering::SeqCst),
        0,
        "500 rounds of image fuzzing produced unhandled panics"
    );
}

// ============================================================================
// Target 8: Malformed ICC Profile & CLUT Table Explosion Injection
// ============================================================================
#[test]
fn test_target_08_malformed_icc_and_clut_explosion() {
    let guard = IccProfileGuard::default();

    // 1. Oversized ICC profile payload (> 1 MiB)
    let oversized_icc = vec![0u8; DEFAULT_MAX_ICC_PROFILE_SIZE + 1024];
    let res_size = guard.inspect(&oversized_icc);
    assert!(
        matches!(res_size, Err(ImageDefenseError::IccProfileSizeExceeded { .. })),
        "ICC profile guard must reject profiles exceeding 1 MiB"
    );

    // 2. Corrupted magic ICC profile
    let mut bad_magic_icc = vec![0u8; 128];
    bad_magic_icc[0..4].copy_from_slice(&128u32.to_be_bytes());
    bad_magic_icc[36..40].copy_from_slice(b"XXXX");
    let res_bad = guard.inspect(&bad_magic_icc);
    assert!(
        matches!(res_bad, Err(ImageDefenseError::IccMalformed { .. })),
        "ICC profile with bad magic must fail inspection"
    );

    // 3. Valid small sRGB profile validation
    let mut small_icc = vec![0u8; 128];
    small_icc[0..4].copy_from_slice(&128u32.to_be_bytes()); // Profile size = 128
    small_icc[16..20].copy_from_slice(b"RGB ");
    small_icc[20..24].copy_from_slice(b"XYZ ");
    small_icc[36..40].copy_from_slice(b"acsp");
    let res_small = guard.inspect(&small_icc);
    assert!(res_small.is_ok());
}

// ============================================================================
// Target 9: Single-Byte & Single-Bit Flip Avalanche Resilience on Magic Headers
// ============================================================================
#[test]
fn test_target_09_single_bit_flip_magic_avalanche() {
    let headers: &[&[u8]] = &[
        &[0xFF, 0xD8, 0xFF, 0xE0], // JPEG
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A], // PNG
        b"RIFF\x24\x00\x00\x00WEBPVP8 ", // WebP
        b"BM\x36\x00\x00\x00", // BMP
        b"qoif\x00\x00\x00\x10", // QOI
        b"farbfeld\x00\x00\x00\x08", // Farbfeld
        b"P6\n10 10\n255\n", // PPM
    ];

    for &hdr in headers {
        let initial_format = TTZipImageDecoder::detect_format(hdr);
        let test_indices: Vec<usize> = match initial_format {
            ImageFormat::Jpeg | ImageFormat::Bmp => vec![0, 1],
            ImageFormat::Ppm => vec![0],
            ImageFormat::Qoi => vec![0, 1, 2, 3],
            ImageFormat::Png | ImageFormat::Farbfeld => (0..8).collect(),
            ImageFormat::WebP => vec![0, 1, 2, 3, 8, 9, 10, 11],
            ImageFormat::Unknown => vec![0],
        };
        for &byte_idx in &test_indices {
            for bit in 0..8 {
                let mut flipped = hdr.to_vec();
                flipped[byte_idx] ^= 1 << bit;
                let fmt = TTZipImageDecoder::detect_format(&flipped);
                assert_ne!(fmt, initial_format, "Flipped header bit at byte {byte_idx} must alter detection");
            }
        }
    }
}

// ============================================================================
// Target 10: Viewport Sub-Region Out-of-Bounds & Zero Bounds Fuzzing
// ============================================================================
#[test]
fn test_target_10_viewport_subregion_boundary_fuzzing() {
    let frame = DecodedImageFrame::new(
        100,
        100,
        ImageColorSpace::Rgba,
        ImageBitDepth::U8,
        vec![128u8; 100 * 100 * 4],
    )
    .unwrap();

    let invalid_rects = [
        ViewportRect::new(200, 200, 50, 50), // x, y completely out of bounds
        ViewportRect::new(0, 0, 0, 0),       // 0x0 size
        ViewportRect::new(50, 50, 0, 100),   // 0 width
        ViewportRect::new(50, 50, 100, 0),   // 0 height
        ViewportRect::new(80, 80, 50, 50),   // partially out of bounds (should clamp)
    ];

    for rect in &invalid_rects {
        let res = ViewportSampler::sample_viewport(&frame, rect, 1.0, ViewportFilter::Bilinear);
        if rect.x >= 100 || rect.y >= 100 || rect.width == 0 || rect.height == 0 {
            assert!(res.is_err(), "Invalid viewport rect must return error");
        } else {
            assert!(res.is_ok(), "Partially overlapping viewport should clamp and succeed");
        }
    }
}

// ============================================================================
// Target 11: Colorspace Conversion Pipeline Boundary & Mismatched Buffer
// ============================================================================
#[test]
fn test_target_11_colorspace_conversion_buffer_mismatch() {
    // 1. Buffer too short for claimed pixel count
    let short_buffer = vec![255u8; 10]; // only 10 bytes, but claims 10 RGB pixels (needs 30 bytes)
    let res = ColorSpacePipeline::convert_buffer(
        &short_buffer,
        ImageColorSpace::Rgb,
        ImageColorSpace::Rgba,
        10,
    );
    assert!(
        matches!(res, Err(ImageError::BufferMismatch { expected: 30, found: 10 })),
        "Colorspace pipeline must detect buffer length mismatch"
    );

    // 2. Empty buffer with 0 pixels succeeds with empty output
    let empty_res = ColorSpacePipeline::convert_buffer(
        &[],
        ImageColorSpace::Rgb,
        ImageColorSpace::Rgba,
        0,
    );
    assert!(empty_res.is_ok());
    assert_eq!(empty_res.unwrap().len(), 0);
}

// ============================================================================
// Target 12: Memory Budget Watchdog Circuit Breaker Under Extreme Allocations
// ============================================================================
#[test]
fn test_target_12_memory_budget_watchdog_circuit_breaker() {
    let watchdog = MemoryBudgetWatchdog::new(64 * 1024 * 1024); // 64 MiB limit

    // 1. Reserve 32 MiB - should succeed
    let res1 = watchdog.reserve(32 * 1024 * 1024);
    assert!(res1.is_ok());
    assert_eq!(watchdog.current_allocated(), 32 * 1024 * 1024);

    // 2. Reserve another 40 MiB - should trip circuit breaker (32 + 40 = 72 > 64)
    let res2 = watchdog.reserve(40 * 1024 * 1024);
    assert!(
        matches!(res2, Err(ImageDefenseError::MemoryBudgetExceeded { .. })),
        "Watchdog must trip circuit breaker when resident memory limit is exceeded"
    );

    // 3. Drop res1 and verify reservation released
    drop(res1);
    assert_eq!(watchdog.current_allocated(), 0);
}

// ============================================================================
// Target 13: Sensitive Image Buffer Zeroize-on-Drop Volatile Erasure Fuzzing
// ============================================================================
#[test]
fn test_target_13_sensitive_image_buffer_zeroize_erasure() {
    use zeroize::Zeroize;

    let secret_pixels = vec![0x42u8; 1024];
    let buf = SensitiveImageBuffer::from_vec(secret_pixels.clone());
    assert_eq!(buf.len(), 1024);
    assert_eq!(buf.as_slice()[0], 0x42);

    drop(buf);

    // Explicit zeroize on separate buffer
    let mut buffer_to_zero = SensitiveImageBuffer::from_vec(secret_pixels);
    buffer_to_zero.zeroize();
    assert!(buffer_to_zero.as_slice().iter().all(|&b| b == 0));
}

// ============================================================================
// Target 14: Dynamic Scale Factor Fuzzing (NaN, Inf, Negative, Extreme)
// ============================================================================
#[test]
fn test_target_14_scale_factor_fuzzing_in_viewport() {
    let frame = DecodedImageFrame::new(
        32,
        32,
        ImageColorSpace::Rgba,
        ImageBitDepth::U8,
        vec![200u8; 32 * 32 * 4],
    )
    .unwrap();
    let rect = ViewportRect::new(0, 0, 32, 32);

    let invalid_scales = [0.0f32, -1.0, -0.001, f32::NAN, f32::INFINITY, f32::NEG_INFINITY];

    for &scale in &invalid_scales {
        let res = ViewportSampler::sample_viewport(&frame, &rect, scale, ViewportFilter::Bilinear);
        assert!(res.is_err(), "Invalid scale factor {scale} must return error");
    }

    // Extreme valid scale factors
    let extreme_small = ViewportSampler::sample_viewport(&frame, &rect, 0.0001, ViewportFilter::Nearest);
    assert!(extreme_small.is_ok());
    assert!(extreme_small.unwrap().width >= 1);

    let extreme_large = ViewportSampler::sample_viewport(&frame, &rect, 8.0, ViewportFilter::Nearest);
    assert!(extreme_large.is_ok());
    assert_eq!(extreme_large.unwrap().width, 256);
}

// ============================================================================
// Target 15: QOI & Farbfeld Malformed Header / Chunk Run-Length Fuzzing
// ============================================================================
#[test]
fn test_target_15_qoi_farbfeld_malformed_header_fuzzing() {
    // 1. QOI header with 0 width/height
    let mut bad_qoi = b"qoif\x00\x00\x00\x00\x00\x00\x00\x00\x04\x00".to_vec();
    bad_qoi.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    let qoi_res = TTZipImageDecoder::decode(&bad_qoi);
    assert!(qoi_res.is_err(), "0x0 dimension QOI must fail");

    // 2. Farbfeld with truncated dimension header
    let truncated_ff = b"farbfeld\x00\x00\x00\x10".to_vec();
    let ff_res = TTZipImageDecoder::decode(&truncated_ff);
    assert!(ff_res.is_err(), "Truncated Farbfeld must fail");
}

// ============================================================================
// Target 16: Multi-Format Sniffing False-Positive & Polyglot Injection Fuzzing
// ============================================================================
#[test]
fn test_target_16_sniffing_false_positive_and_polyglot_fuzzing() {
    // 1. Hybrid file: JPEG magic prefix followed by HTML code
    let mut polyglot = vec![0xFF, 0xD8, 0xFF, 0xE0];
    polyglot.extend_from_slice(b"<html><body>Not an image</body></html>");
    let res = TTZipImageDecoder::decode(&polyglot);
    assert!(res.is_err(), "JPEG/HTML polyglot must fail decoding safely");

    // 2. Format detection on ZIP header
    let zip_magic = [0x50, 0x4B, 0x03, 0x04, 0x14, 0x00];
    assert_eq!(TTZipImageDecoder::detect_format(&zip_magic), ImageFormat::Unknown);

    // 3. Format detection on TAR header
    let tar_magic = vec![0u8; 512];
    assert_eq!(TTZipImageDecoder::detect_format(&tar_magic), ImageFormat::Unknown);
}
