// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust Image Processing and Microkernel Subsystem.
//!
//! Provides ultra-fast multi-format decoding (JPEG, PNG, WebP, QOI, PPM, Farbfeld),
//! zero-allocation Exif thumbnail extraction, dynamic viewport/tile sampling,
//! and SIMD colorspace pipelines with Apple Metal `BGRA8Unorm` texture pass-through.

pub mod colorspace;
pub mod decoder;
pub mod thumbnail;
pub mod viewport;

pub use colorspace::{
    bgr_to_bgra, bgr_to_luma, bgr_to_rgb, bgr_to_rgba, bgra_to_bgr, bgra_to_luma, bgra_to_rgb,
    bgra_to_rgba, luma_to_bgra, luma_to_rgb, luma_to_rgba, lumaa_to_bgra, lumaa_to_luma,
    lumaa_to_rgb, lumaa_to_rgba, rgb_to_bgr, rgb_to_bgra, rgb_to_luma, rgb_to_rgba,
    rgb_to_ycbcr, rgba_to_bgr, rgba_to_bgra, rgba_to_luma, rgba_to_rgb, ycbcr_to_bgra,
    ycbcr_to_luma, ycbcr_to_rgb, ycbcr_to_rgba, ColorSpacePipeline,
};
pub use decoder::{
    DecodedImageFrame, ImageBitDepth, ImageColorSpace, ImageError, ImageFormat, TTZipImageDecoder,
};
pub use thumbnail::{
    calculate_aspect_dimensions as calculate_image_aspect_dimensions, downsample_frame_fast,
    ExifThumbnailExtractor,
};
pub use viewport::{ViewportFilter, ViewportRect, ViewportSampler};

#[cfg(test)]
mod tests {
    use super::*;

    fn create_synthetic_ppm(width: u32, height: u32) -> Vec<u8> {
        let header = format!("P6\n{width} {height}\n255\n");
        let mut data = header.into_bytes();
        for y in 0..height {
            for x in 0..width {
                let r = ((x * 255) / width.max(1)) as u8;
                let g = ((y * 255) / height.max(1)) as u8;
                let b = 128u8;
                data.extend_from_slice(&[r, g, b]);
            }
        }
        data
    }

    fn create_synthetic_farbfeld(width: u32, height: u32) -> Vec<u8> {
        let mut data = Vec::with_capacity(16 + (width * height * 8) as usize);
        data.extend_from_slice(b"farbfeld");
        data.extend_from_slice(&width.to_be_bytes());
        data.extend_from_slice(&height.to_be_bytes());
        for _ in 0..(width * height) {
            // RGBA 16-bit big endian
            data.extend_from_slice(&[255, 0, 0, 255, 0, 0, 255, 255]);
        }
        data
    }

    fn create_synthetic_qoi(width: u32, height: u32) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"qoif");
        data.extend_from_slice(&width.to_be_bytes());
        data.extend_from_slice(&height.to_be_bytes());
        data.push(4); // channels: 4 = RGBA
        data.push(0); // colorspace: sRGB

        // Encode simple QOI_OP_RGBA blocks
        for _ in 0..(width * height) {
            data.push(0b11111111); // QOI_OP_RGBA
            data.extend_from_slice(&[200, 100, 50, 255]);
        }
        // QOI end marker (7 zero bytes followed by 0x01)
        data.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
        data
    }

    #[test]
    fn test_format_detection() {
        let jpeg_magic = [0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(TTZipImageDecoder::detect_format(&jpeg_magic), ImageFormat::Jpeg);

        let png_magic = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(TTZipImageDecoder::detect_format(&png_magic), ImageFormat::Png);

        let webp_magic = b"RIFF\x24\x00\x00\x00WEBPVP8 ";
        assert_eq!(TTZipImageDecoder::detect_format(webp_magic), ImageFormat::WebP);

        let qoi_magic = b"qoif\x00\x00\x00\x10\x00\x00\x00\x10\x04\x00";
        assert_eq!(TTZipImageDecoder::detect_format(qoi_magic), ImageFormat::Qoi);

        let ppm_magic = b"P6\n10 10\n255\n";
        assert_eq!(TTZipImageDecoder::detect_format(ppm_magic), ImageFormat::Ppm);

        let farbfeld_magic = b"farbfeld\x00\x00\x00\x08\x00\x00\x00\x08";
        assert_eq!(TTZipImageDecoder::detect_format(farbfeld_magic), ImageFormat::Farbfeld);

        let bmp_magic = b"BM\x00\x00\x00\x00";
        assert_eq!(TTZipImageDecoder::detect_format(bmp_magic), ImageFormat::Bmp);

        assert_eq!(TTZipImageDecoder::detect_format(&[0, 1, 2]), ImageFormat::Unknown);
    }

    #[test]
    fn test_ppm_decoding_and_rgba_conversion() {
        let ppm_data = create_synthetic_ppm(16, 16);
        let frame = TTZipImageDecoder::decode(&ppm_data).expect("PPM decode failed");
        assert_eq!(frame.width, 16);
        assert_eq!(frame.height, 16);
        assert_eq!(frame.colorspace, ImageColorSpace::Rgb);

        let rgba_frame = TTZipImageDecoder::decode_rgba8(&ppm_data).expect("PPM to RGBA8 failed");
        assert_eq!(rgba_frame.width, 16);
        assert_eq!(rgba_frame.height, 16);
        assert_eq!(rgba_frame.colorspace, ImageColorSpace::Rgba);
        assert_eq!(rgba_frame.bytes.len(), 16 * 16 * 4);
    }

    #[test]
    fn test_farbfeld_decoding() {
        let fb_data = create_synthetic_farbfeld(8, 8);
        let frame = TTZipImageDecoder::decode(&fb_data).expect("Farbfeld decode failed");
        assert_eq!(frame.width, 8);
        assert_eq!(frame.height, 8);
    }

    #[test]
    fn test_qoi_decoding() {
        let qoi_data = create_synthetic_qoi(4, 4);
        let frame = TTZipImageDecoder::decode(&qoi_data).expect("QOI decode failed");
        assert_eq!(frame.width, 4);
        assert_eq!(frame.height, 4);
    }

    #[test]
    fn test_colorspace_pipeline_conversions() {
        let rgb_src = vec![255, 128, 64, 10, 20, 30]; // 2 pixels RGB
        let rgba = rgb_to_rgba(&rgb_src, 255);
        assert_eq!(rgba, vec![255, 128, 64, 255, 10, 20, 30, 255]);

        let bgr = rgb_to_bgr(&rgb_src);
        assert_eq!(bgr, vec![64, 128, 255, 30, 20, 10]);

        let bgra = rgb_to_bgra(&rgb_src, 200);
        assert_eq!(bgra, vec![64, 128, 255, 200, 30, 20, 10, 200]);

        let back_rgb = rgba_to_rgb(&rgba);
        assert_eq!(back_rgb, rgb_src);

        let bgra_from_rgba = rgba_to_bgra(&rgba);
        assert_eq!(bgra_from_rgba, vec![64, 128, 255, 255, 30, 20, 10, 255]);

        let back_rgba = bgra_to_rgba(&bgra_from_rgba);
        assert_eq!(back_rgba, rgba);

        let luma = rgb_to_luma(&rgb_src);
        assert_eq!(luma.len(), 2);

        let luma_rgba = luma_to_rgba(&luma, 255);
        assert_eq!(luma_rgba.len(), 8);
    }

    #[test]
    fn test_ycbcr_conversions() {
        let rgb_src = vec![255, 0, 0, 0, 255, 0, 0, 0, 255]; // Red, Green, Blue
        let ycbcr = rgb_to_ycbcr(&rgb_src);
        assert_eq!(ycbcr.len(), 9);

        let back_rgb = ycbcr_to_rgb(&ycbcr);
        assert_eq!(back_rgb.len(), 9);
        // Ensure color reconstruction is reasonably close (within standard rounding tolerances)
        assert!((back_rgb[0] as i32 - 255).abs() <= 5);
        assert!((back_rgb[1] as i32) <= 5);
        assert!((back_rgb[2] as i32) <= 5);
    }

    #[test]
    fn test_apple_metal_bgra8_passthrough() {
        let frame = DecodedImageFrame::new(
            2,
            2,
            ImageColorSpace::Rgba,
            ImageBitDepth::U8,
            vec![
                255, 0, 0, 255,   // Red
                0, 255, 0, 255,   // Green
                0, 0, 255, 255,   // Blue
                255, 255, 255, 255, // White
            ],
        )
        .expect("Valid frame");

        let metal_bytes = ColorSpacePipeline::to_metal_bgra8(&frame).expect("Metal conversion");
        assert_eq!(
            metal_bytes,
            vec![
                0, 0, 255, 255,   // BGRA (B=0, G=0, R=255, A=255)
                0, 255, 0, 255,   // BGRA (B=0, G=255, R=0, A=255)
                255, 0, 0, 255,   // BGRA (B=255, G=0, R=0, A=255)
                255, 255, 255, 255,
            ]
        );
    }

    #[test]
    fn test_viewport_sampler_and_tiles() {
        let width = 100u32;
        let height = 100u32;
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                pixels.extend_from_slice(&[x as u8, y as u8, 128, 255]);
            }
        }

        let source = DecodedImageFrame::new(
            width,
            height,
            ImageColorSpace::Rgba,
            ImageBitDepth::U8,
            pixels,
        )
        .expect("Valid source frame");

        // Test crop 1:1
        let rect = ViewportRect::new(10, 10, 20, 20);
        let cropped = ViewportSampler::sample_viewport(&source, &rect, 1.0, ViewportFilter::Nearest)
            .expect("Viewport sampling failed");
        assert_eq!(cropped.width, 20);
        assert_eq!(cropped.height, 20);

        // Verify top-left pixel matches (10, 10)
        assert_eq!(cropped.bytes[0..4], [10, 10, 128, 255]);

        // Test downsampled viewport with Bilinear
        let scaled_down =
            ViewportSampler::sample_viewport(&source, &rect, 0.5, ViewportFilter::Bilinear)
                .expect("Downsample failed");
        assert_eq!(scaled_down.width, 10);
        assert_eq!(scaled_down.height, 10);

        // Test tile generation
        let tile = ViewportSampler::generate_tile(
            &source,
            0,
            0,
            32,
            1.0,
            ViewportFilter::AreaAverage,
        )
        .expect("Tile generation failed");
        assert_eq!(tile.width, 32);
        assert_eq!(tile.height, 32);

        // Test tile grid calculation
        let grid = ViewportSampler::compute_tile_grid(100, 100, 32);
        assert_eq!(grid.len(), 16); // 4x4 tiles
    }

    #[test]
    fn test_exif_thumbnail_synthetic_extraction() {
        // Construct a synthetic JPEG APP1 header with TIFF IFD0 -> IFD1 embedded JPEG
        let thumb_jpeg = vec![
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01,
            0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xD9,
        ];

        let mut tiff = Vec::new();
        // TIFF Header: Little Endian ("II"), 42, offset to IFD0 = 8
        tiff.extend_from_slice(b"II");
        tiff.extend_from_slice(&42u16.to_le_bytes());
        tiff.extend_from_slice(&8u32.to_le_bytes());

        // IFD0: 0 entries, next IFD offset points to offset 14
        let ifd0_offset = tiff.len();
        assert_eq!(ifd0_offset, 8);
        tiff.extend_from_slice(&0u16.to_le_bytes()); // 0 entries
        let ifd1_offset = 14u32;
        tiff.extend_from_slice(&ifd1_offset.to_le_bytes());

        // IFD1 at offset 14: 2 entries
        // Entry 1: Tag 0x0201 (thumb offset), Type 4 (LONG), Count 1, Value = offset to thumb
        // Entry 2: Tag 0x0202 (thumb length), Type 4 (LONG), Count 1, Value = length of thumb
        // Next IFD = 0 (4 bytes)
        // Then thumb bytes
        assert_eq!(tiff.len(), 14);
        tiff.extend_from_slice(&2u16.to_le_bytes()); // 2 entries

        let thumb_data_offset = (14 + 2 + 12 * 2 + 4) as u32;
        let thumb_data_len = thumb_jpeg.len() as u32;

        // Entry 1 (0x0201)
        tiff.extend_from_slice(&0x0201u16.to_le_bytes());
        tiff.extend_from_slice(&4u16.to_le_bytes()); // LONG
        tiff.extend_from_slice(&1u32.to_le_bytes()); // Count 1
        tiff.extend_from_slice(&thumb_data_offset.to_le_bytes());

        // Entry 2 (0x0202)
        tiff.extend_from_slice(&0x0202u16.to_le_bytes());
        tiff.extend_from_slice(&4u16.to_le_bytes()); // LONG
        tiff.extend_from_slice(&1u32.to_le_bytes()); // Count 1
        tiff.extend_from_slice(&thumb_data_len.to_le_bytes());

        // Next IFD offset = 0
        tiff.extend_from_slice(&0u32.to_le_bytes());

        // Append embedded thumbnail
        assert_eq!(tiff.len(), thumb_data_offset as usize);
        tiff.extend_from_slice(&thumb_jpeg);

        // Wrap into JPEG APP1 payload
        let mut jpeg = vec![0xFF, 0xD8]; // SOI
        let app1_len = (2 + 6 + tiff.len()) as u16;
        jpeg.extend_from_slice(&[0xFF, 0xE1]); // APP1
        jpeg.extend_from_slice(&app1_len.to_be_bytes());
        jpeg.extend_from_slice(b"Exif\0\0");
        jpeg.extend_from_slice(&tiff);
        jpeg.extend_from_slice(&[0xFF, 0xD9]); // EOI

        let extracted = ExifThumbnailExtractor::extract_embedded_jpeg(&jpeg);
        assert!(extracted.is_some());
        assert_eq!(extracted.unwrap(), thumb_jpeg.as_slice());
    }

    #[test]
    fn test_fast_downsample_fallback() {
        let frame = DecodedImageFrame::new(
            64,
            64,
            ImageColorSpace::Rgb,
            ImageBitDepth::U8,
            vec![128u8; 64 * 64 * 3],
        )
        .expect("Valid frame");

        let downsampled = downsample_frame_fast(&frame, 16, 16);
        assert_eq!(downsampled.width, 16);
        assert_eq!(downsampled.height, 16);
        assert_eq!(downsampled.bytes.len(), 16 * 16 * 3);
        assert_eq!(downsampled.bytes[0], 128);
    }

    #[test]
    fn test_real_jpeg_decode_and_thumbnail() {
        let width = 32u16;
        let height = 32u16;
        let mut pixels = Vec::with_capacity((width * height * 3) as usize);
        for y in 0..height {
            for x in 0..width {
                pixels.extend_from_slice(&[x as u8 * 7, y as u8 * 7, 100]);
            }
        }

        let mut jpeg_bytes = Vec::new();
        let encoder = jpeg_encoder::Encoder::new(&mut jpeg_bytes, 85);
        encoder
            .encode(&pixels, width, height, jpeg_encoder::ColorType::Rgb)
            .expect("JPEG encode failed");

        // Format detection
        assert_eq!(TTZipImageDecoder::detect_format(&jpeg_bytes), ImageFormat::Jpeg);

        // Decode native frame
        let frame = TTZipImageDecoder::decode(&jpeg_bytes).expect("JPEG decode failed");
        assert_eq!(frame.width, 32);
        assert_eq!(frame.height, 32);

        // Decode BGRA8 (Apple Metal format)
        let bgra_frame = TTZipImageDecoder::decode_bgra8(&jpeg_bytes).expect("JPEG decode BGRA8 failed");
        assert_eq!(bgra_frame.width, 32);
        assert_eq!(bgra_frame.height, 32);
        assert_eq!(bgra_frame.colorspace, ImageColorSpace::Bgra);

        // Extract or generate thumbnail
        let thumb = ExifThumbnailExtractor::extract_or_generate(&jpeg_bytes, 16, 16)
            .expect("Thumbnail extraction failed");
        assert!(thumb.width <= 16);
        assert!(thumb.height <= 16);
    }

    #[test]
    fn test_error_handling() {
        assert_eq!(
            TTZipImageDecoder::decode(&[]),
            Err(ImageError::EmptyData)
        );

        let corrupt_data = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0xFF, 0xFF];
        assert!(TTZipImageDecoder::decode(&corrupt_data).is_err());

        let invalid_rect = ViewportRect::new(200, 200, 50, 50);
        let frame = DecodedImageFrame::new(
            10,
            10,
            ImageColorSpace::Rgb,
            ImageBitDepth::U8,
            vec![0u8; 300],
        )
        .expect("Valid frame");

        assert!(ViewportSampler::sample_viewport(&frame, &invalid_rect, 1.0, ViewportFilter::Nearest).is_err());
    }
}
