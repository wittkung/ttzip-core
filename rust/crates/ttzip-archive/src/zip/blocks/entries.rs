// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Local file headers, central directory entries, and data descriptors.

use crate::to_and_from_le;

// =============================================================================
// 1. ZipLocalEntryBlock (26 Bytes)
// =============================================================================

to_and_from_le! {
    /// ZIP Local File Header fixed-size payload (26 bytes).
    ///
    /// Follows the 4-byte signature `MAGIC_LFH` (`0x04034B50`).
    pub struct ZipLocalEntryBlock {
        magic: 0x04034B50,
        options: ZipLocalEntryBlockOptions,
        pub version_needed: u16,
        pub general_purpose_flag: u16,
        pub compression_method: u16,
        pub last_mod_time: u16,
        pub last_mod_date: u16,
        pub crc32: u32,
        pub compressed_size: u32,
        pub uncompressed_size: u32,
        pub file_name_length: u16,
        pub extra_field_length: u16,
    }
}

/// Convenience type alias for [`ZipLocalEntryBlockOptions`].
pub type ZipBlockOptions = ZipLocalEntryBlockOptions;

// =============================================================================
// 2. ZipCentralEntryBlock (42 Bytes)
// =============================================================================

to_and_from_le! {
    /// ZIP Central Directory File Header (CDFH) fixed-size payload (42 bytes).
    ///
    /// Follows the 4-byte signature `MAGIC_CDFH` (`0x02014B50`).
    pub struct ZipCentralEntryBlock {
        magic: 0x02014B50,
        options: ZipCentralEntryBlockOptions,
        pub version_made_by: u16,
        pub version_needed: u16,
        pub general_purpose_flag: u16,
        pub compression_method: u16,
        pub last_mod_time: u16,
        pub last_mod_date: u16,
        pub crc32: u32,
        pub compressed_size: u32,
        pub uncompressed_size: u32,
        pub file_name_length: u16,
        pub extra_field_length: u16,
        pub file_comment_length: u16,
        pub disk_number_start: u16,
        pub internal_file_attributes: u16,
        pub external_file_attributes: u32,
        pub relative_offset_of_local_header: u32,
    }
}

// =============================================================================
// 3. ZipDataDescriptorBlock (12 Bytes)
// =============================================================================

to_and_from_le! {
    /// ZIP 32-bit Data Descriptor fixed-size block (12 bytes).
    ///
    /// Appended after compressed stream when bit 3 of general purpose flag is set.
    /// May optionally be preceded by the 4-byte signature `MAGIC_DATA_DESCRIPTOR` (`0x08074B50`).
    pub struct ZipDataDescriptorBlock {
        magic: 0x08074B50,
        pub crc32: u32,
        pub compressed_size: u32,
        pub uncompressed_size: u32,
    }
}

// =============================================================================
// 4. Zip64DataDescriptorBlock (20 Bytes)
// =============================================================================

to_and_from_le! {
    /// ZIP64 Data Descriptor fixed-size block (20 bytes).
    ///
    /// Appended after compressed stream for Zip64 entries when bit 3 of general purpose flag is set.
    /// May optionally be preceded by the 4-byte signature `MAGIC_DATA_DESCRIPTOR` (`0x08074B50`).
    pub struct Zip64DataDescriptorBlock {
        magic: 0x08074B50,
        pub crc32: u32,
        pub compressed_size: u64,
        pub uncompressed_size: u64,
    }
}
