// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! `spk!` binary delta archive container serialization, deserialization, and compression.
//!
//! Encapsulates format magic validation, before/after tree topology hashes,
//! compressed control triplets, diff stream, extra stream, and command tables.

use crate::checksum::crc32;
use crate::system::delta::bsdiff::{BsDiffControl, BsDiffPatch};
use crate::system::delta::types::{DeltaCommand, DeltaError, DeltaPatchHeader, DeltaResult};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::{Read, Write};

/// Modern TTZip delta archive container (`spk!`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TTZipDeltaArchive {
    /// Container header metadata.
    pub header: DeltaPatchHeader,
    /// Compressed control triplet stream bytes.
    pub compressed_controls: Vec<u8>,
    /// Compressed additive diff stream bytes.
    pub compressed_diff: Vec<u8>,
    /// Compressed literal extra stream bytes.
    pub compressed_extra: Vec<u8>,
    /// Compressed metadata commands table.
    pub compressed_commands: Vec<u8>,
}

impl TTZipDeltaArchive {
    /// Canonical magic bytes for TTZip delta format v4.
    pub const MAGIC_SPK4: [u8; 4] = *b"spk!";
    /// Major version 4.
    pub const MAJOR_VERSION_4: u16 = 4;
    /// Minor version 0.
    pub const MINOR_VERSION_0: u16 = 0;

    /// Builds a new `TTZipDeltaArchive` from raw patch components and commands.
    pub fn create(
        before_tree_hash: u32,
        after_tree_hash: u32,
        uncompressed_size: u64,
        patch: &BsDiffPatch,
        commands: &[DeltaCommand],
    ) -> DeltaResult<Self> {
        let header = DeltaPatchHeader::new(
            Self::MAGIC_SPK4,
            Self::MAJOR_VERSION_4,
            Self::MINOR_VERSION_0,
            before_tree_hash,
            after_tree_hash,
            uncompressed_size,
        );

        // Serialize controls: count followed by fixed 24-byte structs
        let mut raw_controls = Vec::with_capacity(4 + patch.controls.len() * 24);
        raw_controls.extend_from_slice(&(patch.controls.len() as u32).to_le_bytes());
        for ctrl in &patch.controls {
            raw_controls.extend_from_slice(&(ctrl.diff_len as u64).to_le_bytes());
            raw_controls.extend_from_slice(&(ctrl.extra_len as u64).to_le_bytes());
            raw_controls.extend_from_slice(&ctrl.seek_offset.to_le_bytes());
        }

        let compressed_controls = compress_zlib(&raw_controls)?;
        let compressed_diff = compress_zlib(&patch.diff_data)?;
        let compressed_extra = compress_zlib(&patch.extra_data)?;

        let raw_commands = serde_json::to_vec(commands)
            .map_err(|e| DeltaError::CodecError(format!("Command serialization error: {}", e)))?;
        let compressed_commands = compress_zlib(&raw_commands)?;

        Ok(Self {
            header,
            compressed_controls,
            compressed_diff,
            compressed_extra,
            compressed_commands,
        })
    }

    /// Serializes the container into a byte buffer with trailing CRC-32 integrity guard.
    pub fn serialize(&self) -> DeltaResult<Vec<u8>> {
        let mut buf = Vec::with_capacity(
            DeltaPatchHeader::HEADER_SIZE
                + 16
                + self.compressed_controls.len()
                + self.compressed_diff.len()
                + self.compressed_extra.len()
                + self.compressed_commands.len()
                + 4,
        );

        // 1. Header (24 bytes)
        buf.extend_from_slice(&self.header.to_bytes());

        // 2. Section lengths & payloads
        write_chunk(&mut buf, &self.compressed_controls);
        write_chunk(&mut buf, &self.compressed_diff);
        write_chunk(&mut buf, &self.compressed_extra);
        write_chunk(&mut buf, &self.compressed_commands);

        // 3. CRC-32 Checksum of entire container payload
        let checksum = crc32(0, &buf);
        buf.extend_from_slice(&checksum.to_le_bytes());

        Ok(buf)
    }

    /// Deserializes and validates a delta archive container from bytes.
    pub fn deserialize(bytes: &[u8]) -> DeltaResult<Self> {
        if bytes.len() < DeltaPatchHeader::HEADER_SIZE + 16 + 4 {
            return Err(DeltaError::TruncatedData {
                needed: DeltaPatchHeader::HEADER_SIZE + 20,
                available: bytes.len(),
            });
        }

        // Validate trailing CRC32
        let payload_len = bytes.len() - 4;
        let expected_crc = u32::from_le_bytes([
            bytes[payload_len],
            bytes[payload_len + 1],
            bytes[payload_len + 2],
            bytes[payload_len + 3],
        ]);
        let computed_crc = crc32(0, &bytes[..payload_len]);
        if expected_crc != computed_crc {
            return Err(DeltaError::CorruptedPatch(format!(
                "Container CRC32 mismatch: expected {expected_crc:#010x}, computed {computed_crc:#010x}"
            )));
        }

        // 1. Parse header
        let header = DeltaPatchHeader::from_bytes(&bytes[..DeltaPatchHeader::HEADER_SIZE])?;
        if header.magic != Self::MAGIC_SPK4 && header.magic != *b"SPK4" {
            return Err(DeltaError::InvalidMagic(header.magic));
        }

        let mut offset = DeltaPatchHeader::HEADER_SIZE;

        // 2. Read sections
        let compressed_controls = read_chunk(bytes, &mut offset)?;
        let compressed_diff = read_chunk(bytes, &mut offset)?;
        let compressed_extra = read_chunk(bytes, &mut offset)?;
        let compressed_commands = read_chunk(bytes, &mut offset)?;

        Ok(Self {
            header,
            compressed_controls,
            compressed_diff,
            compressed_extra,
            compressed_commands,
        })
    }

    /// Decompresses and reconstructs the `BsDiffPatch` streams.
    pub fn decompress_patch(&self) -> DeltaResult<BsDiffPatch> {
        let raw_controls = decompress_zlib(&self.compressed_controls)?;
        if raw_controls.len() < 4 {
            return Err(DeltaError::CorruptedPatch("Missing controls count".into()));
        }

        let count = u32::from_le_bytes([
            raw_controls[0],
            raw_controls[1],
            raw_controls[2],
            raw_controls[3],
        ]) as usize;

        let expected_size = 4 + count * 24;
        if raw_controls.len() < expected_size {
            return Err(DeltaError::TruncatedData {
                needed: expected_size,
                available: raw_controls.len(),
            });
        }

        let mut controls = Vec::with_capacity(count);
        let mut ptr = 4;
        for _ in 0..count {
            let diff_len = u64::from_le_bytes(raw_controls[ptr..ptr + 8].try_into().unwrap()) as usize;
            ptr += 8;
            let extra_len = u64::from_le_bytes(raw_controls[ptr..ptr + 8].try_into().unwrap()) as usize;
            ptr += 8;
            let seek_offset = i64::from_le_bytes(raw_controls[ptr..ptr + 8].try_into().unwrap());
            ptr += 8;

            controls.push(BsDiffControl::new(diff_len, extra_len, seek_offset));
        }

        let diff_data = decompress_zlib(&self.compressed_diff)?;
        let extra_data = decompress_zlib(&self.compressed_extra)?;

        Ok(BsDiffPatch {
            controls,
            diff_data,
            extra_data,
        })
    }

    /// Decompresses the metadata commands list.
    pub fn decompress_commands(&self) -> DeltaResult<Vec<DeltaCommand>> {
        let raw = decompress_zlib(&self.compressed_commands)?;
        if raw.is_empty() {
            return Ok(Vec::new());
        }
        serde_json::from_slice(&raw)
            .map_err(|e| DeltaError::CodecError(format!("Command deserialization error: {}", e)))
    }
}

#[inline]
fn write_chunk(buf: &mut Vec<u8>, data: &[u8]) {
    buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
    buf.extend_from_slice(data);
}

#[inline]
fn read_chunk<'a>(bytes: &'a [u8], offset: &mut usize) -> DeltaResult<Vec<u8>> {
    if *offset + 4 > bytes.len() {
        return Err(DeltaError::TruncatedData {
            needed: *offset + 4,
            available: bytes.len(),
        });
    }

    let len = u32::from_le_bytes([
        bytes[*offset],
        bytes[*offset + 1],
        bytes[*offset + 2],
        bytes[*offset + 3],
    ]) as usize;
    *offset += 4;

    if *offset + len > bytes.len() {
        return Err(DeltaError::TruncatedData {
            needed: *offset + len,
            available: bytes.len(),
        });
    }

    let slice = &bytes[*offset..*offset + len];
    *offset += len;
    Ok(slice.to_vec())
}

#[inline]
fn compress_zlib(data: &[u8]) -> DeltaResult<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    encoder
        .write_all(data)
        .map_err(|e| DeltaError::CodecError(e.to_string()))?;
    encoder
        .finish()
        .map_err(|e| DeltaError::CodecError(e.to_string()))
}

#[inline]
fn decompress_zlib(compressed: &[u8]) -> DeltaResult<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(compressed);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| DeltaError::CodecError(e.to_string()))?;
    Ok(decompressed)
}
