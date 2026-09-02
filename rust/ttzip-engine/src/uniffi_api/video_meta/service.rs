// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Service and Pipeline Implementations for Video Probing, Metadata, and Cover Art Extraction.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use super::parser::{extract_video_cover_from_bytes, parse_video_metadata_from_bytes};
use super::types::{UniFFIVideoError, UniFFIVideoMetadata};

// ============================================================================
// Helper I/O Functions
// ============================================================================

/// Reads file bytes from a local filesystem path with error mapping.
fn read_file_bytes(file_path: &str) -> Result<Vec<u8>, UniFFIVideoError> {
    let mut file = File::open(file_path).map_err(|e| UniFFIVideoError::IoError {
        message: format!("Failed to open video file '{file_path}': {e}"),
    })?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|e| UniFFIVideoError::IoError {
            message: format!("Failed to read video file '{file_path}': {e}"),
        })?;
    Ok(buffer)
}

// ============================================================================
// Exported Free Functions
// ============================================================================

/// Probes technical stream parameters and container properties from in-memory video bytes without full decoding.
#[uniffi::export]
pub fn uniffi_probe_video_bytes(
    data: Vec<u8>,
    file_name: Option<String>,
) -> Result<UniFFIVideoMetadata, UniFFIVideoError> {
    parse_video_metadata_from_bytes(&data, file_name.as_deref())
}

/// Extracts comprehensive metadata tags, track topology, and cover art info from in-memory video bytes.
#[uniffi::export]
pub fn uniffi_extract_video_metadata(
    data: Vec<u8>,
    file_name: Option<String>,
) -> Result<UniFFIVideoMetadata, UniFFIVideoError> {
    parse_video_metadata_from_bytes(&data, file_name.as_deref())
}

/// Extracts raw embedded poster or cover art image bytes from in-memory video bytes.
#[uniffi::export]
pub fn uniffi_extract_video_cover(
    data: Vec<u8>,
    file_name: Option<String>,
) -> Result<Vec<u8>, UniFFIVideoError> {
    extract_video_cover_from_bytes(&data, file_name.as_deref())
}

// ============================================================================
// Stateful UniFFI Service Object
// ============================================================================

/// High-performance Mozilla UniFFI video engine service exposing probing, track topology, and cover art extraction.
#[derive(uniffi::Object, Default)]
pub struct UniFFIVideoService {}

#[uniffi::export]
impl UniFFIVideoService {
    /// Constructs a new thread-safe video service instance.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {})
    }

    /// Probes technical video metadata from an in-memory byte buffer.
    pub fn probe_bytes(
        &self,
        data: Vec<u8>,
        file_name: Option<String>,
    ) -> Result<UniFFIVideoMetadata, UniFFIVideoError> {
        parse_video_metadata_from_bytes(&data, file_name.as_deref())
    }

    /// Probes technical video metadata from a local video file on disk.
    pub fn probe_file(&self, file_path: String) -> Result<UniFFIVideoMetadata, UniFFIVideoError> {
        let bytes = read_file_bytes(&file_path)?;
        let name = Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
        parse_video_metadata_from_bytes(&bytes, name.as_deref())
    }

    /// Extracts comprehensive video metadata from an in-memory byte buffer.
    pub fn extract_metadata(
        &self,
        data: Vec<u8>,
        file_name: Option<String>,
    ) -> Result<UniFFIVideoMetadata, UniFFIVideoError> {
        parse_video_metadata_from_bytes(&data, file_name.as_deref())
    }

    /// Extracts comprehensive video metadata from a local video file on disk.
    pub fn extract_metadata_from_file(
        &self,
        file_path: String,
    ) -> Result<UniFFIVideoMetadata, UniFFIVideoError> {
        let bytes = read_file_bytes(&file_path)?;
        let name = Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
        parse_video_metadata_from_bytes(&bytes, name.as_deref())
    }

    /// Extracts raw embedded cover art bytes from an in-memory byte buffer.
    pub fn extract_cover(
        &self,
        data: Vec<u8>,
        file_name: Option<String>,
    ) -> Result<Vec<u8>, UniFFIVideoError> {
        extract_video_cover_from_bytes(&data, file_name.as_deref())
    }

    /// Extracts raw embedded cover art bytes from a local video file on disk.
    pub fn extract_cover_from_file(&self, file_path: String) -> Result<Vec<u8>, UniFFIVideoError> {
        let bytes = read_file_bytes(&file_path)?;
        let name = Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
        extract_video_cover_from_bytes(&bytes, name.as_deref())
    }
}
