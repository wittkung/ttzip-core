// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! End of Central Directory (EOCD) records and Zip64 locator blocks.

use crate::to_and_from_le;

// =============================================================================
// 5. Zip32CDEBlock (18 Bytes)
// =============================================================================

to_and_from_le! {
    /// ZIP End of Central Directory (EOCD) fixed-size payload (18 bytes).
    ///
    /// Follows the 4-byte signature `MAGIC_EOCD` (`0x06054B50`).
    pub struct Zip32CDEBlock {
        magic: 0x06054B50,
        pub disk_number: u16,
        pub disk_with_central_directory: u16,
        pub total_entries_this_disk: u16,
        pub total_entries: u16,
        pub central_directory_size: u32,
        pub central_directory_offset: u32,
        pub comment_length: u16,
    }
}

// =============================================================================
// 6. Zip64CDELocatorBlock (16 Bytes)
// =============================================================================

to_and_from_le! {
    /// ZIP64 End of Central Directory Locator fixed-size payload (16 bytes).
    ///
    /// Follows the 4-byte signature `MAGIC_ZIP64_LOCATOR` (`0x07064B50`).
    pub struct Zip64CDELocatorBlock {
        magic: 0x07064B50,
        pub disk_with_zip64_central_directory: u32,
        pub zip64_central_directory_offset: u64,
        pub total_number_of_disks: u32,
    }
}

// =============================================================================
// 7. Zip64CDEBlock (52 Bytes)
// =============================================================================

to_and_from_le! {
    /// ZIP64 End of Central Directory Record fixed-size payload (52 bytes).
    ///
    /// Follows the 4-byte signature `MAGIC_ZIP64_EOCD` (`0x06064B50`).
    pub struct Zip64CDEBlock {
        magic: 0x06064B50,
        options: Zip64CDEBlockOptions,
        pub record_size: u64,
        pub version_made_by: u16,
        pub version_needed: u16,
        pub disk_number: u32,
        pub disk_with_central_directory: u32,
        pub total_entries_this_disk: u64,
        pub total_entries: u64,
        pub central_directory_size: u64,
        pub central_directory_offset: u64,
    }
}
