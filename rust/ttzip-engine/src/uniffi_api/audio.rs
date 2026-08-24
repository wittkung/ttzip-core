// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI Audio Waveform Extraction Scaffolding.

use super::types::TTZipError;

/// Extracts normalized audio waveform amplitudes from a file on disk.
#[uniffi::export]
pub fn extract_audio_waveform(path: String, bucket_count: u32) -> Result<Vec<f32>, TTZipError> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path });
    }
    crate::audio::extract_waveform_from_file(p, bucket_count as usize)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })
}

/// Extracts normalized audio waveform amplitudes from memory data.
#[uniffi::export]
pub fn extract_audio_waveform_from_memory(data: Vec<u8>, bucket_count: u32) -> Result<Vec<f32>, TTZipError> {
    Ok(crate::audio::extract_waveform_from_bytes(&data, bucket_count as usize))
}
