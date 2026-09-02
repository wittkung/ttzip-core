// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Data types, commands, container headers, and error definitions for binary delta patching.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Binary delta patch format identifiers supported by the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeltaFormat {
    /// BSDIFF40 standard legacy differential patch format.
    Bsdiff40,
    /// BSDIFN40 differential patch format with native block compression.
    Bsdifn40,
    /// TTZip modern stream patch format version 3.
    Spk3,
    /// TTZip modern high-throughput stream patch container version 4 (`spk!`).
    Spk4,
    /// Unknown or unsupported patch format.
    Unknown,
}

impl DeltaFormat {
    /// Detects the delta patch format from the leading header bytes.
    #[inline]
    pub fn from_magic(magic: &[u8]) -> Self {
        if magic.len() < 4 {
            return Self::Unknown;
        }
        match &magic[0..4] {
            b"BSDI" => {
                if magic.len() >= 8 && &magic[0..8] == b"BSDIFF40" {
                    Self::Bsdiff40
                } else if magic.len() >= 8 && &magic[0..8] == b"BSDIFN40" {
                    Self::Bsdifn40
                } else {
                    Self::Bsdiff40
                }
            }
            b"SPK3" => Self::Spk3,
            b"SPK4" | b"spk!" => Self::Spk4,
            _ => Self::Unknown,
        }
    }

    /// Returns the canonical magic bytes for the format.
    #[inline]
    pub const fn magic_bytes(&self) -> &'static [u8] {
        match self {
            Self::Bsdiff40 => b"BSDIFF40",
            Self::Bsdifn40 => b"BSDIFN40",
            Self::Spk3 => b"SPK3",
            Self::Spk4 => b"spk!",
            Self::Unknown => b"UNKN",
        }
    }
}

/// Structural delta patch commands for granular file and chunk mutations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeltaCommand {
    /// Extract payload or diff target at specified byte offset.
    Extract {
        /// Starting offset in the target payload stream.
        offset: u64,
        /// Number of payload bytes to extract.
        length: u64,
    },
    /// Delete byte region or tree entity.
    Delete {
        /// Starting offset in the target stream.
        offset: u64,
        /// Number of bytes to delete.
        length: u64,
    },
    /// Binary differential chunk with additive diff stream and extra stream.
    BinaryDiff {
        /// Number of bytes to read from the additive diff stream.
        diff_len: usize,
        /// Number of bytes to read from the literal extra stream.
        extra_len: usize,
        /// Signed seek displacement in the source buffer relative to the current position.
        seek_offset: i64,
    },
    /// Modify permissions or UNIX metadata mode.
    ModifyPermissions {
        /// Target UNIX file mode bitmask (e.g. 0o755).
        mode: u32,
    },
    /// Clone byte block or file range without transferring redundant data.
    Clone {
        /// Source byte offset in the base image.
        source_offset: u64,
        /// Destination byte offset in the target image.
        target_offset: u64,
        /// Number of bytes to duplicate.
        length: u64,
    },
}

/// Header for `spk!` and TTZip binary delta patch containers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(C)]
pub struct DeltaPatchHeader {
    /// Magic identifier bytes (e.g. `b"spk!"` or `b"SPK4"`).
    pub magic: [u8; 4],
    /// Major format version (e.g. 4 for `spk!`).
    pub major_version: u16,
    /// Minor format version (e.g. 0).
    pub minor_version: u16,
    /// Tree topology hash of source data before patching.
    pub before_tree_hash: u32,
    /// Tree topology hash of target data after patching.
    pub after_tree_hash: u32,
    /// Uncompressed final reconstructed target size in bytes.
    pub uncompressed_size: u64,
}

impl DeltaPatchHeader {
    /// Total binary serialized length of the fixed container header (24 bytes).
    pub const HEADER_SIZE: usize = 24;

    /// Creates a new delta patch header.
    #[inline]
    pub const fn new(
        magic: [u8; 4],
        major_version: u16,
        minor_version: u16,
        before_tree_hash: u32,
        after_tree_hash: u32,
        uncompressed_size: u64,
    ) -> Self {
        Self {
            magic,
            major_version,
            minor_version,
            before_tree_hash,
            after_tree_hash,
            uncompressed_size,
        }
    }

    /// Serializes the header into a fixed 24-byte little-endian array.
    #[inline]
    pub fn to_bytes(&self) -> [u8; Self::HEADER_SIZE] {
        let mut buf = [0u8; Self::HEADER_SIZE];
        buf[0..4].copy_from_slice(&self.magic);
        buf[4..6].copy_from_slice(&self.major_version.to_le_bytes());
        buf[6..8].copy_from_slice(&self.minor_version.to_le_bytes());
        buf[8..12].copy_from_slice(&self.before_tree_hash.to_le_bytes());
        buf[12..16].copy_from_slice(&self.after_tree_hash.to_le_bytes());
        buf[16..24].copy_from_slice(&self.uncompressed_size.to_le_bytes());
        buf
    }

    /// Deserializes a header from a 24-byte slice.
    #[inline]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DeltaError> {
        if bytes.len() < Self::HEADER_SIZE {
            return Err(DeltaError::TruncatedData {
                needed: Self::HEADER_SIZE,
                available: bytes.len(),
            });
        }

        let mut magic = [0u8; 4];
        magic.copy_from_slice(&bytes[0..4]);

        let major_version = u16::from_le_bytes([bytes[4], bytes[5]]);
        let minor_version = u16::from_le_bytes([bytes[6], bytes[7]]);
        let before_tree_hash = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let after_tree_hash = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let uncompressed_size = u64::from_le_bytes([
            bytes[16], bytes[17], bytes[18], bytes[19], bytes[20], bytes[21], bytes[22], bytes[23],
        ]);

        Ok(Self {
            magic,
            major_version,
            minor_version,
            before_tree_hash,
            after_tree_hash,
            uncompressed_size,
        })
    }
}

/// Execution telemetry and statistics for delta patch application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeltaPatchResult {
    /// Input compressed patch bytes consumed.
    pub bytes_in: usize,
    /// Output reconstructed target bytes produced.
    pub bytes_out: usize,
    /// Total number of delta instructions / triplet blocks applied.
    pub instructions_applied: usize,
    /// SHA-256 hexadecimal digest of the reconstructed target.
    pub sha256_hex: String,
}

/// Error domain for delta patching, diffing, and archive serialization.
#[derive(Debug, Error)]
pub enum DeltaError {
    /// The patch file header does not contain a recognized magic signature.
    #[error("Invalid delta patch magic header: {0:?}")]
    InvalidMagic([u8; 4]),

    /// The patch format version is not supported by this engine runtime.
    #[error("Unsupported delta patch version: {major}.{minor}")]
    UnsupportedVersion {
        /// Encountered major version.
        major: u16,
        /// Encountered minor version.
        minor: u16,
    },

    /// The source data tree hash does not match the header expectation.
    #[error("Source data tree hash mismatch: expected {expected:#010x}, actual {actual:#010x}")]
    SourceHashMismatch {
        /// Expected topology hash.
        expected: u32,
        /// Actual calculated hash.
        actual: u32,
    },

    /// The reconstructed target data tree hash does not match the header expectation.
    #[error("Target data tree hash mismatch: expected {expected:#010x}, actual {actual:#010x}")]
    TargetHashMismatch {
        /// Expected topology hash.
        expected: u32,
        /// Actual calculated hash.
        actual: u32,
    },

    /// The binary patch payload is corrupted or contains invalid stream values.
    #[error("Corrupted delta patch stream: {0}")]
    CorruptedPatch(String),

    /// The patch payload ends prematurely before expected structures could be read.
    #[error("Truncated delta patch data: needed {needed} bytes, only {available} available")]
    TruncatedData {
        /// Minimum byte count required.
        needed: usize,
        /// Byte count actually available in stream.
        available: usize,
    },

    /// Writing to the target buffer would exceed maximum declared capacity.
    #[error("Target buffer overflow: requested write size {requested} exceeds buffer limit {capacity}")]
    TargetBufferOverflow {
        /// Requested byte size.
        requested: usize,
        /// Allocated capacity.
        capacity: usize,
    },

    /// The relative seek displacement points outside valid source buffer bounds.
    #[error("Out of bounds seek displacement: offset {offset}, boundary {boundary}")]
    OutOfBoundsSeek {
        /// Target offset calculation.
        offset: i64,
        /// Maximum allowable slice boundary.
        boundary: usize,
    },

    /// Compression or decompression codec failure during container processing.
    #[error("Compression/Decompression error: {0}")]
    CodecError(String),

    /// Underlying standard I/O stream error.
    #[error("I/O error during delta operation: {0}")]
    IoError(#[from] std::io::Error),
}

/// Specialized Result type for delta operations.
pub type DeltaResult<T> = Result<T, DeltaError>;
