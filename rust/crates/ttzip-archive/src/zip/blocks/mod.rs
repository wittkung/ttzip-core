// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Native ZIP Fixed-Size Binary Block SerDe and Endian Conversion.
//!
//! Provides zero-copy Plain Old Data (`Pod`) transmutation, compile-time fixed-size
//! layout verification, and declarative little-endian pack/unpack macros (`to_and_from_le!`)
//! for the 7 core ZIP header structures.

pub mod entries;
pub mod eocd;
#[macro_use]
pub mod macros;
pub mod traits;

pub use entries::{
    ZipBlockOptions, ZipCentralEntryBlock, ZipCentralEntryBlockOptions,
    ZipDataDescriptorBlock, Zip64DataDescriptorBlock,
    ZipLocalEntryBlock, ZipLocalEntryBlockOptions,
};
pub use eocd::{Zip32CDEBlock, Zip64CDEBlock, Zip64CDEBlockOptions, Zip64CDELocatorBlock};
pub use macros::to_and_from_le;
pub use traits::{FixedSizeBlock, Pod};

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zip_local_entry_block_options_construction() {
        let opt = ZipLocalEntryBlockOptions {
            version_needed: 20,
            general_purpose_flag: 0x0800,
            compression_method: 8,
            last_mod_time: 0x1234,
            last_mod_date: 0x5678,
            crc32: 0xDEADBEEF,
            compressed_size: 100,
            uncompressed_size: 200,
            file_name_length: 12,
            extra_field_length: 0,
        };
        let block = ZipLocalEntryBlock::new(opt);
        let extracted = ZipLocalEntryBlockOptions::from(block);
        assert_eq!(extracted.version_needed, 20);
        assert_eq!(extracted.crc32, 0xDEADBEEF);
        assert_eq!(extracted.compressed_size, 100);
        assert_eq!(extracted.uncompressed_size, 200);
    }

    #[test]
    fn test_zip_local_entry_block_options_roundtrip() {
        let opt = ZipLocalEntryBlockOptions {
            version_needed: 45,
            general_purpose_flag: 0,
            compression_method: 0,
            last_mod_time: 1,
            last_mod_date: 2,
            crc32: 3,
            compressed_size: 4,
            uncompressed_size: 5,
            file_name_length: 6,
            extra_field_length: 7,
        };
        let block = ZipLocalEntryBlock::from_options(opt);
        let back_opt: ZipLocalEntryBlockOptions = block.into();
        assert_eq!(opt, back_opt);
    }

    #[test]
    fn test_zip_block_options_alias() {
        let opt: ZipBlockOptions = ZipBlockOptions::default();
        let block = ZipLocalEntryBlock::new(opt);
        let extracted = ZipBlockOptions::from(block);
        assert_eq!(extracted.version_needed, 0);
    }

    #[test]
    fn test_zip_central_entry_block_options_construction() {
        let opt = ZipCentralEntryBlockOptions {
            version_made_by: 0x031E,
            version_needed: 20,
            general_purpose_flag: 0x0800,
            compression_method: 8,
            last_mod_time: 0x1111,
            last_mod_date: 0x2222,
            crc32: 0x12345678,
            compressed_size: 500,
            uncompressed_size: 1000,
            file_name_length: 16,
            extra_field_length: 4,
            file_comment_length: 0,
            disk_number_start: 0,
            internal_file_attributes: 0,
            external_file_attributes: 0o644 << 16,
            relative_offset_of_local_header: 1024,
        };
        let block = ZipCentralEntryBlock::new(opt);
        let extracted = ZipCentralEntryBlockOptions::from(block);
        assert_eq!(extracted.version_made_by, 0x031E);
        assert_eq!(extracted.relative_offset_of_local_header, 1024);
    }

    #[test]
    fn test_zip_central_entry_block_options_roundtrip() {
        let opt = ZipCentralEntryBlockOptions {
            version_made_by: 1,
            version_needed: 2,
            general_purpose_flag: 3,
            compression_method: 4,
            last_mod_time: 5,
            last_mod_date: 6,
            crc32: 7,
            compressed_size: 8,
            uncompressed_size: 9,
            file_name_length: 10,
            extra_field_length: 11,
            file_comment_length: 12,
            disk_number_start: 13,
            internal_file_attributes: 14,
            external_file_attributes: 15,
            relative_offset_of_local_header: 16,
        };
        let block = ZipCentralEntryBlock::from_options(opt);
        let back_opt: ZipCentralEntryBlockOptions = block.into();
        assert_eq!(opt, back_opt);
    }

    #[test]
    fn test_zip_64_cde_block_options_construction() {
        let opt = Zip64CDEBlockOptions {
            record_size: 44,
            version_made_by: 45,
            version_needed: 45,
            disk_number: 0,
            disk_with_central_directory: 0,
            total_entries_this_disk: 100000,
            total_entries: 100000,
            central_directory_size: 5000000,
            central_directory_offset: 10000000,
        };
        let block = Zip64CDEBlock::new(opt);
        let extracted = Zip64CDEBlockOptions::from(block);
        assert_eq!(extracted.record_size, 44);
        assert_eq!(extracted.total_entries, 100000);
        assert_eq!(extracted.central_directory_offset, 10000000);
    }

    #[test]
    fn test_zip_64_cde_block_options_roundtrip() {
        let opt = Zip64CDEBlockOptions {
            record_size: 1,
            version_made_by: 2,
            version_needed: 3,
            disk_number: 4,
            disk_with_central_directory: 5,
            total_entries_this_disk: 6,
            total_entries: 7,
            central_directory_size: 8,
            central_directory_offset: 9,
        };
        let block = Zip64CDEBlock::from_options(opt);
        let back_opt: Zip64CDEBlockOptions = block.into();
        assert_eq!(opt, back_opt);
    }

    #[test]
    fn test_zip_data_descriptor_block_positional_new() {
        let dd = ZipDataDescriptorBlock::new(0x12345678, 100, 200);
        let expected = ZipDataDescriptorBlock {
            crc32: 0x12345678,
            compressed_size: 100,
            uncompressed_size: 200,
        };
        assert_eq!(dd, expected);
    }

    #[test]
    fn test_zip_64_data_descriptor_block_positional_new() {
        let dd = Zip64DataDescriptorBlock::new(0x87654321, 1000000000, 2000000000);
        let expected = Zip64DataDescriptorBlock {
            crc32: 0x87654321,
            compressed_size: 1000000000,
            uncompressed_size: 2000000000,
        };
        assert_eq!(dd, expected);
    }

    #[test]
    fn test_zip_32_cde_block_positional_new() {
        let cde = Zip32CDEBlock::new(0, 0, 5, 5, 250, 1000, 0);
        let expected = Zip32CDEBlock {
            disk_number: 0,
            disk_with_central_directory: 0,
            total_entries_this_disk: 5,
            total_entries: 5,
            central_directory_size: 250,
            central_directory_offset: 1000,
            comment_length: 0,
        };
        assert_eq!(cde, expected);
    }

    #[test]
    fn test_zip_64_cde_locator_block_positional_new() {
        let loc = Zip64CDELocatorBlock::new(0, 50000, 1);
        let expected = Zip64CDELocatorBlock {
            disk_with_zip64_central_directory: 0,
            zip64_central_directory_offset: 50000,
            total_number_of_disks: 1,
        };
        assert_eq!(loc, expected);
    }

    #[test]
    fn test_zip_local_entry_block_from_into_options() {
        let opt = ZipLocalEntryBlockOptions::default();
        let block: ZipLocalEntryBlock = opt.into();
        let opt_again: ZipLocalEntryBlockOptions = block.into();
        assert_eq!(opt, opt_again);
    }

    #[test]
    fn test_zip_central_entry_block_from_into_options() {
        let opt = ZipCentralEntryBlockOptions::default();
        let block: ZipCentralEntryBlock = opt.into();
        let opt_again: ZipCentralEntryBlockOptions = block.into();
        assert_eq!(opt, opt_again);
    }

    #[test]
    fn test_zip_64_cde_block_from_into_options() {
        let opt = Zip64CDEBlockOptions::default();
        let block: Zip64CDEBlock = opt.into();
        let opt_again: Zip64CDEBlockOptions = block.into();
        assert_eq!(opt, opt_again);
    }
}
