// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Strongly-typed ZIP Type-Length-Value (TLV) Extra Field parser and serializer.
//!
//! Provides comprehensive support for the 7 major ZIP extra field families:
//! 1. Zip64 Extended Information (`TAG_ZIP64 = 0x0001`)
//! 2. NTFS 100ns Timestamps (`TAG_NTFS = 0x000a`)
//! 3. Extended Timestamp (`TAG_EXT_TIMESTAMP = 0x5455`)
//! 4. Info-ZIP Unicode Comment (`TAG_UNICODE_COMMENT = 0x6375`)
//! 5. Info-ZIP Unicode Path (`TAG_UNICODE_PATH = 0x7075`)
//! 6. ASi Unix Metadata & Symlinks (`TAG_ASI_UNIX = 0x756e`)
//! 7. Info-ZIP Unix New 32-bit UID/GID (`TAG_INFOZIP_UNIX_NEW = 0x7875`)
//! 8. WinZip AES Encryption (`TAG_WINZIP_AES = 0x9901`)
//! 9. Data Stream Alignment (`TAG_DATA_STREAM_ALIGNMENT = 0xa11e`)

pub mod builder;
pub mod parser;
pub mod types;

pub use parser::ExtraFieldsParser;
pub use types::*;

/// Consolidated Extra Fields parsed metadata.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ZipExtraFields {
    // Strongly typed family representations
    pub zip64: Option<Zip64Extra>,
    pub extended_timestamp: Option<ExtendedTimestampExtra>,
    pub infozip_unix_new: Option<InfoZipUnixNewExtra>,
    pub ntfs: Option<NtfsExtra>,
    pub unicode_path: Option<UnicodeFieldExtra>,
    pub unicode_comment: Option<UnicodeFieldExtra>,
    pub winzip_aes: Option<WinZipAesExtra>,
    pub asi_unix: Option<AsiUnixExtra>,
    pub alignment: Option<DataStreamAlignmentExtra>,
    pub unknown_fields: Vec<(u16, Vec<u8>)>,

    // Flat legacy-compatible fields
    pub has_zip64: bool,
    pub uncompressed_size: Option<u64>,
    pub compressed_size: Option<u64>,
    pub local_header_offset: Option<u64>,
    pub disk_number: Option<u32>,

    pub has_winzip_aes: bool,
    pub aes_version: u16,
    pub aes_vendor_id: u16,
    pub aes_strength: u8,
    pub aes_actual_method: u16,

    pub has_extended_timestamp: bool,
    pub mod_time: Option<u32>,
    pub acc_time: Option<u32>,
    pub create_time: Option<u32>,

    pub has_posix_permissions: bool,
    pub uid: Option<u32>,
    pub gid: Option<u32>,

    pub unicode_path_str: Option<String>,
    pub data_stream_alignment: Option<u16>,
}

impl ZipExtraFields {
    /// Parses extra fields from a raw slice with backward-compatible placeholder arguments.
    pub fn parse(
        extra_data: &[u8],
        is_cdfh: bool,
        uncomp_size_placeholder: bool,
        comp_size_placeholder: bool,
        lfh_offset_placeholder: bool,
    ) -> Self {
        ExtraFieldsParser::parse_with_placeholders(
            extra_data,
            is_cdfh,
            uncomp_size_placeholder,
            comp_size_placeholder,
            lfh_offset_placeholder,
        )
    }
}
