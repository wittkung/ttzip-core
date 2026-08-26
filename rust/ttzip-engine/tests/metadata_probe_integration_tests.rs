// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

#![cfg(feature = "probe")]

use std::fs;
use tempfile::tempdir;
use ttzip_engine::uniffi_api::metadata::{
    probe_buffer_metadata, probe_file_metadata, FileMediaType,
};

#[test]
fn test_uniffi_probe_jpeg_buffer() {
    let mut jpeg = Vec::new();
    jpeg.extend_from_slice(&[0xFF, 0xD8]); // SOI

    // SOF0: 3840 x 2160
    jpeg.extend_from_slice(&[0xFF, 0xC0]);
    jpeg.extend_from_slice(&17u16.to_be_bytes());
    jpeg.push(8); // 8-bit
    jpeg.extend_from_slice(&2160u16.to_be_bytes()); // height
    jpeg.extend_from_slice(&3840u16.to_be_bytes()); // width
    jpeg.push(3);
    jpeg.extend_from_slice(&[1, 0x11, 0, 2, 0x11, 1, 3, 0x11, 1]);
    jpeg.extend_from_slice(&[0xFF, 0xD9]); // EOI

    let record = probe_buffer_metadata(jpeg, Some("photo_4k.jpg".to_string())).expect("Probe successful");
    assert_eq!(record.media_type, FileMediaType::Image);
    assert_eq!(record.format_name, "JPEG Image");
    assert_eq!(record.mime_type, "image/jpeg");

    let img = record.image.expect("Image metadata present");
    assert_eq!(img.width, 3840);
    assert_eq!(img.height, 2160);
    assert_eq!(img.bit_depth, 8);
    assert!(!img.has_alpha);
    assert_eq!(record.attributes.get("Dimensions").map(|s| s.as_str()), Some("3840 × 2160"));
}

#[test]
fn test_uniffi_probe_png_buffer() {
    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&1920u32.to_be_bytes());
    ihdr.extend_from_slice(&1080u32.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(6); // RGBA
    ihdr.push(0);
    ihdr.push(0);
    ihdr.push(0);

    png.extend_from_slice(&(ihdr.len() as u32).to_be_bytes());
    png.extend_from_slice(b"IHDR");
    png.extend_from_slice(&ihdr);
    png.extend_from_slice(&[0, 0, 0, 0]);

    png.extend_from_slice(&0u32.to_be_bytes());
    png.extend_from_slice(b"IEND");
    png.extend_from_slice(&[0xAE, 0x42, 0x60, 0x82]);

    let record = probe_buffer_metadata(png, Some("banner.png".to_string())).expect("Probe successful");
    assert_eq!(record.media_type, FileMediaType::Image);
    assert_eq!(record.format_name, "PNG Image");
    assert_eq!(record.mime_type, "image/png");

    let img = record.image.expect("Image metadata present");
    assert_eq!(img.width, 1920);
    assert_eq!(img.height, 1080);
    assert!(img.has_alpha);
}

#[test]
fn test_uniffi_probe_wav_audio() {
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&44u32.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&2u16.to_le_bytes()); // 2 channels
    wav.extend_from_slice(&44100u32.to_le_bytes()); // 44.1 kHz
    wav.extend_from_slice(&176400u32.to_le_bytes()); // byte rate
    wav.extend_from_slice(&4u16.to_le_bytes()); // block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // 16-bit
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&882000u32.to_le_bytes()); // 882000 bytes = 5 seconds

    let record = probe_buffer_metadata(wav, Some("recording.wav".to_string())).expect("Probe successful");
    assert_eq!(record.media_type, FileMediaType::Audio);
    assert_eq!(record.format_name, "WAV Audio");

    let aud = record.audio.expect("Audio metadata present");
    assert_eq!(aud.sample_rate, 44100);
    assert_eq!(aud.channels, 2);
    assert_eq!(aud.bit_depth, 16);
    assert_eq!(aud.duration_secs, 5.0);
}

#[test]
fn test_uniffi_probe_3d_stl_file_mmap() {
    let dir = tempdir().expect("Create temp dir");
    let file_path = dir.path().join("cube.stl");

    // Binary STL with 12 triangles (standard cube)
    let mut stl = vec![0u8; 84 + 12 * 50];
    stl[0..80].fill(b'C');
    stl[80..84].copy_from_slice(&12u32.to_le_bytes());

    fs::write(&file_path, &stl).expect("Write file");

    let record = probe_file_metadata(file_path.to_str().unwrap().to_string()).expect("Probe file");
    assert_eq!(record.media_type, FileMediaType::Model3D);
    assert_eq!(record.file_size, stl.len() as u64);

    let m3d = record.model_3d.expect("3D metadata present");
    assert_eq!(m3d.triangle_count, Some(12));
    assert_eq!(m3d.vertex_count, Some(36));
}

#[test]
fn test_uniffi_probe_non_existent_file() {
    let err = probe_file_metadata("/non/existent/path/for/sure.xyz".to_string());
    assert!(err.is_err());
}
