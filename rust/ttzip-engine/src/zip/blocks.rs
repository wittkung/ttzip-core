// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Native ZIP Fixed-Size Binary Block SerDe and Endian Conversion.
//!
//! Provides zero-copy Plain Old Data (`Pod`) transmutation, compile-time fixed-size
//! layout verification, and declarative little-endian pack/unpack macros (`to_and_from_le!`)
//! for the 7 core ZIP header structures.

/// Marker trait for Plain Old Data (POD) types that can safely undergo zero-copy
/// serialization and deserialization from raw byte slices.
///
/// # Safety
///
/// Implementors must ensure that:
/// 1. The type has a well-defined packed memory layout (`#[repr(C, packed)]`).
/// 2. All bit patterns within the type's byte footprint are valid representations (e.g., primitive integers).
/// 3. The type does not implement `Drop` and contains no pointers, references, or uninitialized padding.
pub unsafe trait Pod: Copy + 'static {
    /// Casts an immutable byte slice prefix to a reference of `Self` if the slice is large enough.
    ///
    /// Returns a tuple containing the reference to `Self` and the remaining unconsumed slice.
    #[inline(always)]
    fn ref_from_prefix(bytes: &[u8]) -> Option<(&Self, &[u8])> {
        let size = std::mem::size_of::<Self>();
        if bytes.len() < size {
            return None;
        }
        let (head, tail) = bytes.split_at(size);
        let ptr = head.as_ptr() as *const Self;
        unsafe { Some((&*ptr, tail)) }
    }

    /// Casts a mutable byte slice prefix to a mutable reference of `Self` if the slice is large enough.
    ///
    /// Returns a tuple containing the mutable reference to `Self` and the remaining unconsumed slice.
    #[inline(always)]
    fn mut_from_prefix(bytes: &mut [u8]) -> Option<(&mut Self, &mut [u8])> {
        let size = std::mem::size_of::<Self>();
        if bytes.len() < size {
            return None;
        }
        let (head, tail) = bytes.split_at_mut(size);
        let ptr = head.as_mut_ptr() as *mut Self;
        unsafe { Some((&mut *ptr, tail)) }
    }

    /// Returns a raw byte slice view of `self`.
    #[inline(always)]
    fn as_bytes(&self) -> &[u8] {
        let size = std::mem::size_of::<Self>();
        unsafe { std::slice::from_raw_parts(self as *const Self as *const u8, size) }
    }

    /// Returns a mutable raw byte slice view of `self`.
    #[inline(always)]
    fn as_mut_bytes(&mut self) -> &mut [u8] {
        let size = std::mem::size_of::<Self>();
        unsafe { std::slice::from_raw_parts_mut(self as *mut Self as *mut u8, size) }
    }
}

/// Common trait for all fixed-size ZIP binary structures.
pub trait FixedSizeBlock: Pod + Sized {
    /// 4-byte signature magic constant for this fixed-size block.
    const MAGIC: u32;

    /// Fixed size in bytes of this block in raw binary form.
    const SIZE: usize = std::mem::size_of::<Self>();

    /// Converts all numerical fields in `self` from little-endian representation to host-endian.
    #[allow(clippy::wrong_self_convention)]
    fn from_le(self) -> Self;

    /// Converts all numerical fields in `self` from host-endian representation to little-endian.
    fn to_le(self) -> Self;

    /// Parses a block from a raw byte slice, performing endian conversion from little-endian.
    ///
    /// Returns the parsed block and the number of bytes consumed (`Self::SIZE`).
    #[inline]
    fn parse(bytes: &[u8]) -> Option<(Self, usize)> {
        let size = std::mem::size_of::<Self>();
        if bytes.len() < size {
            return None;
        }
        let mut val = std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), val.as_mut_ptr() as *mut u8, size);
            Some((val.assume_init().from_le(), size))
        }
    }

    /// Parses a block from the prefix of a raw byte slice, returning the parsed block
    /// and the remaining unconsumed slice.
    #[inline]
    fn parse_from_prefix(bytes: &[u8]) -> Option<(Self, &[u8])> {
        let size = std::mem::size_of::<Self>();
        if bytes.len() < size {
            return None;
        }
        let (head, tail) = bytes.split_at(size);
        let mut val = std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            std::ptr::copy_nonoverlapping(head.as_ptr(), val.as_mut_ptr() as *mut u8, size);
            Some((val.assume_init().from_le(), tail))
        }
    }

    /// Serializes `self` into little-endian format and writes it into `dest`.
    ///
    /// Returns the number of bytes written (`Self::SIZE`) or `None` if `dest` is too small.
    #[inline]
    fn write(&self, dest: &mut [u8]) -> Option<usize> {
        let size = std::mem::size_of::<Self>();
        if dest.len() < size {
            return None;
        }
        let le_self = self.to_le();
        unsafe {
            std::ptr::copy_nonoverlapping(
                &le_self as *const Self as *const u8,
                dest.as_mut_ptr(),
                size,
            );
        }
        Some(size)
    }

    /// Serializes `self` into little-endian format and appends it to a `Vec<u8>`.
    #[inline]
    fn write_to_vec(&self, out: &mut Vec<u8>) {
        let size = std::mem::size_of::<Self>();
        let le_self = self.to_le();
        let start = out.len();
        out.reserve(size);
        unsafe {
            let p = out.as_mut_ptr().add(start);
            std::ptr::copy_nonoverlapping(&le_self as *const Self as *const u8, p, size);
            out.set_len(start + size);
        }
    }
}

/// Declarative macro to define fixed-size binary structures with automatic `Pod`, `FixedSizeBlock`,
/// and endian conversion implementation.
#[macro_export]
macro_rules! to_and_from_le {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            magic: $magic:expr,
            $(
                $(#[$field_meta:meta])*
                $fvis:vis $field:ident : $type:ident
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[repr(C, packed)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        $vis struct $name {
            $(
                $(#[$field_meta])*
                $fvis $field : $type,
            )*
        }

        unsafe impl $crate::zip::blocks::Pod for $name {}

        impl $name {
            /// Fixed byte size of the block.
            pub const SIZE: usize = std::mem::size_of::<Self>();

            /// 4-byte signature magic constant for this block.
            pub const MAGIC: u32 = $magic;

            /// Constructs a new instance of the block with the given field values.
            #[allow(clippy::too_many_arguments)]
            #[inline(always)]
            pub const fn new($( $field : $type ),*) -> Self {
                Self {
                    $( $field, )*
                }
            }

            /// Converts all numerical fields from little-endian to host-endian.
            #[allow(clippy::wrong_self_convention)]
            #[inline(always)]
            pub fn from_le(self) -> Self {
                Self {
                    $(
                        $field: $type::from_le(self.$field),
                    )*
                }
            }

            /// Converts all numerical fields from host-endian to little-endian.
            #[inline(always)]
            pub fn to_le(self) -> Self {
                Self {
                    $(
                        $field: $type::to_le(self.$field),
                    )*
                }
            }
        }

        impl $crate::zip::blocks::FixedSizeBlock for $name {
            const MAGIC: u32 = $magic;

            #[inline(always)]
            fn from_le(self) -> Self {
                self.from_le()
            }

            #[inline(always)]
            fn to_le(self) -> Self {
                self.to_le()
            }
        }
    };
}

pub use to_and_from_le;

// =============================================================================
// 1. ZipLocalEntryBlock (26 Bytes)
// =============================================================================

to_and_from_le! {
    /// ZIP Local File Header fixed-size payload (26 bytes).
    ///
    /// Follows the 4-byte signature `MAGIC_LFH` (`0x04034B50`).
    pub struct ZipLocalEntryBlock {
        magic: 0x04034B50,
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

// =============================================================================
// 2. ZipCentralEntryBlock (42 Bytes)
// =============================================================================

to_and_from_le! {
    /// ZIP Central Directory File Header (CDFH) fixed-size payload (42 bytes).
    ///
    /// Follows the 4-byte signature `MAGIC_CDFH` (`0x02014B50`).
    pub struct ZipCentralEntryBlock {
        magic: 0x02014B50,
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
