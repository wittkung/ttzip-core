// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Constants and error types for AppleDouble and macOS metadata handling.

use std::fmt;

/// Standard AppleDouble Magic Number (`0x00051607`).
pub const APPLEDOUBLE_MAGIC: u32 = 0x0005_1607;

/// Standard AppleSingle Magic Number (`0x00051600`).
pub const APPLESINGLE_MAGIC: u32 = 0x0005_1600;

/// AppleDouble / AppleSingle Format Version 2.0 (`0x00020000`).
pub const APPLEDOUBLE_VERSION_2: u32 = 0x0002_0000;

/// Header base size without entry descriptors (26 bytes).
pub const APPLEDOUBLE_HEADER_BASE_SIZE: usize = 26;

/// Entry descriptor size (12 bytes).
pub const APPLEDOUBLE_ENTRY_DESCRIPTOR_SIZE: usize = 12;

/// Standard Finder Info size (32 bytes: 16-byte FInfo + 16-byte FXInfo).
pub const FINDER_INFO_SIZE: usize = 32;

/// Default Home File System filler (16 ASCII bytes).
pub const DEFAULT_HOME_FS: &[u8; 16] = b"Mac OS X        ";

/// AppleDouble Entry ID: Data Fork.
pub const ENTRY_DATA_FORK: u32 = 1;
/// AppleDouble Entry ID: Resource Fork.
pub const ENTRY_RESOURCE_FORK: u32 = 2;
/// AppleDouble Entry ID: Real Name.
pub const ENTRY_REAL_NAME: u32 = 3;
/// AppleDouble Entry ID: Comment.
pub const ENTRY_COMMENT: u32 = 4;
/// AppleDouble Entry ID: Black & White Icon.
pub const ENTRY_ICON_BW: u32 = 5;
/// AppleDouble Entry ID: Color Icon.
pub const ENTRY_ICON_COLOR: u32 = 6;
/// AppleDouble Entry ID: File Dates Info.
pub const ENTRY_FILE_DATES: u32 = 8;
/// AppleDouble Entry ID: Finder Info (32 bytes).
pub const ENTRY_FINDER_INFO: u32 = 9;
/// AppleDouble Entry ID: Mac Drawing.
pub const ENTRY_MAC_DRAWING: u32 = 10;
/// AppleDouble Entry ID: Non-Macintosh Data.
pub const ENTRY_NON_MAC_DATA: u32 = 11;
/// AppleDouble Entry ID: High-level Document Type.
pub const ENTRY_HIGH_LEVEL_DOC_TYPE: u32 = 12;
/// AppleDouble Entry ID: Total File Size.
pub const ENTRY_TOTAL_FILE_SIZE: u32 = 13;
/// AppleDouble Entry ID: Finder Info / Extended Attributes.
pub const ENTRY_FINDER_ATTRS: u32 = 14;

/// Extended attribute key for Finder Info.
pub const XATTR_FINDER_INFO: &str = "com.apple.FinderInfo";
/// Extended attribute key for Resource Fork.
pub const XATTR_RESOURCE_FORK: &str = "com.apple.ResourceFork";
/// Extended attribute key for Quarantine metadata.
pub const XATTR_QUARANTINE: &str = "com.apple.quarantine";

/// Error conditions in AppleDouble header decoding or metadata conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacMetadataError {
    /// Buffer is smaller than the minimum required header size.
    BufferTooShort { required: usize, actual: usize },
    /// Invalid magic number in header.
    InvalidMagic(u32),
    /// Unsupported format version.
    UnsupportedVersion(u32),
    /// Entry descriptor points outside the buffer range.
    OffsetOutOfBounds { offset: u32, length: u32, buffer_len: usize },
    /// Finder Info entry has invalid payload length.
    InvalidFinderInfoLength(usize),
    /// UTF-8 decode error in real name or quarantine string.
    Utf8Error(String),
}

impl fmt::Display for MacMetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooShort { required, actual } => {
                write!(f, "Buffer too short for AppleDouble: need {} bytes, got {}", required, actual)
            }
            Self::InvalidMagic(m) => write!(f, "Invalid AppleDouble magic: 0x{:08X}", m),
            Self::UnsupportedVersion(v) => write!(f, "Unsupported AppleDouble version: 0x{:08X}", v),
            Self::OffsetOutOfBounds { offset, length, buffer_len } => write!(
                f,
                "AppleDouble entry out of bounds: offset {} len {} exceeds total buffer {}",
                offset, length, buffer_len
            ),
            Self::InvalidFinderInfoLength(len) => {
                write!(f, "Invalid FinderInfo length: {} (expected 32)", len)
            }
            Self::Utf8Error(msg) => write!(f, "UTF-8 conversion error: {}", msg),
        }
    }
}

impl std::error::Error for MacMetadataError {}
