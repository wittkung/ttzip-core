// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Dual-mode match table implementation for LZMA2 match-finder acceleration.
//!
//! Provides ultra-compact and scalable match-history recording:
//! - `BitPackedEntry` (32-bit, `#[repr(transparent)]`): Combines 26-bit link offset
//!   (addressing up to 64 MB dictionary) and 6-bit match length (0..=63) into a single `u32`.
//! - `StructuredMatchEntry` (40-bit, `#[repr(C, packed)]`): 32-bit link offset
//!   (addressing >64 MB up to 1 GB+ dictionary) and 8-bit match length (0..=255).
//! - `MatchTable`: Polymorphic dual-mode container with jump-pointer chaining for extended
//!   matches and zero-copy byte buffer borrowing for direct I/O and hardware engine pipelines.

use std::fmt;

/// Maximum dictionary size threshold (64 MB) eligible for 26/6-bit compact bit-packing.
pub const COMPACT_DICT_THRESHOLD: usize = 64 * 1024 * 1024;

/// Upper limit on traversal depth during jump-pointer chain expansion to guard against cycles.
pub const MAX_JUMP_CHAIN_HOPS: usize = 65536;

/// Transparent 32-bit bit-packed match entry for dictionaries <= 64 MB.
///
/// Bit Layout:
/// - Bits [0..25]  (26 bits): Link offset (0..=67,108,863).
/// - Bits [26..31] (6 bits) : Match length (0..=63).
#[repr(transparent)]
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct BitPackedEntry(pub u32);

impl BitPackedEntry {
    /// Number of bits allocated for the link offset.
    pub const LINK_BITS: u32 = 26;

    /// Number of bits allocated for the match length.
    pub const LENGTH_BITS: u32 = 6;

    /// Bitmask for isolating the 26-bit link offset (`0x03FF_FFFF`).
    pub const LINK_MASK: u32 = (1 << Self::LINK_BITS) - 1;

    /// Maximum link offset representable in 26 bits (67,108,863).
    pub const MAX_LINK: u32 = Self::LINK_MASK;

    /// Bit shift offset for the match length field.
    pub const LENGTH_SHIFT: u32 = Self::LINK_BITS;

    /// Bitmask for isolating the 6-bit match length (`0x3F`).
    pub const LENGTH_MASK: u32 = (1 << Self::LENGTH_BITS) - 1;

    /// Maximum match length representable in a single 6-bit field (63).
    pub const MAX_LENGTH: u32 = Self::LENGTH_MASK;

    /// Creates a new `BitPackedEntry` from link offset and match length.
    ///
    /// Values exceeding their respective maximum bit-depths are safely masked.
    #[inline(always)]
    pub const fn new(link: u32, length: u32) -> Self {
        let packed = ((length & Self::LENGTH_MASK) << Self::LENGTH_SHIFT)
            | (link & Self::LINK_MASK);
        Self(packed)
    }

    /// Constructs an entry directly from a raw 32-bit word.
    #[inline(always)]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw 32-bit packed word.
    #[inline(always)]
    pub const fn raw(&self) -> u32 {
        self.0
    }

    /// Extracts the 26-bit link offset (0..=67,108,863).
    #[inline(always)]
    pub const fn link(&self) -> u32 {
        self.0 & Self::LINK_MASK
    }

    /// Extracts the 6-bit match length (0..=63).
    #[inline(always)]
    pub const fn length(&self) -> u32 {
        (self.0 >> Self::LENGTH_SHIFT) & Self::LENGTH_MASK
    }

    /// Updates the 26-bit link offset while preserving the current match length.
    #[inline(always)]
    pub fn set_link(&mut self, link: u32) {
        self.0 = (self.0 & !Self::LINK_MASK) | (link & Self::LINK_MASK);
    }

    /// Updates the 6-bit match length while preserving the current link offset.
    #[inline(always)]
    pub fn set_length(&mut self, length: u32) {
        let shifted_len = (length & Self::LENGTH_MASK) << Self::LENGTH_SHIFT;
        self.0 = (self.0 & Self::LINK_MASK) | shifted_len;
    }

    /// Updates both link offset and match length atomically.
    #[inline(always)]
    pub fn set(&mut self, link: u32, length: u32) {
        self.0 = ((length & Self::LENGTH_MASK) << Self::LENGTH_SHIFT)
            | (link & Self::LINK_MASK);
    }
}

impl fmt::Debug for BitPackedEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BitPackedEntry")
            .field("link", &self.link())
            .field("length", &self.length())
            .field("raw", &format_args!("0x{:08X}", self.0))
            .finish()
    }
}

impl fmt::Display for BitPackedEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BitPacked(link={}, len={})", self.link(), self.length())
    }
}

/// Packed 40-bit structured match entry for large dictionaries (>64 MB up to 1 GB+).
///
/// Memory Layout (5 bytes packed, zero padding):
/// - Bytes [0..3] (32 bits, little-endian): Link offset (0..=4,294,967,295).
/// - Byte  [4]    (8 bits)                 : Match length (0..=255).
#[repr(C, packed)]
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct StructuredMatchEntry {
    /// 32-bit link offset addressing up to 4 GB window.
    pub link: u32,
    /// 8-bit match length (0..=255).
    pub length: u8,
}

impl StructuredMatchEntry {
    /// Byte size of each packed entry in memory (5 bytes).
    pub const ENTRY_SIZE: usize = 5;

    /// Maximum link offset representable in 32 bits (`u32::MAX`).
    pub const MAX_LINK: u32 = u32::MAX;

    /// Maximum match length representable in a single 8-bit field (255).
    pub const MAX_LENGTH: u32 = 255;

    /// Creates a new `StructuredMatchEntry`.
    #[inline(always)]
    pub const fn new(link: u32, length: u8) -> Self {
        Self { link, length }
    }

    /// Reads the 32-bit link offset safely without unaligned reference hazards.
    #[inline(always)]
    pub fn link(&self) -> u32 {
        unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(self.link)) }
    }

    /// Reads the 8-bit match length as `u32`.
    #[inline(always)]
    pub fn length(&self) -> u32 {
        unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(self.length)) as u32 }
    }

    /// Reads the 8-bit match length directly as `u8`.
    #[inline(always)]
    pub fn length_u8(&self) -> u8 {
        unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(self.length)) }
    }

    /// Sets both link and length safely without unaligned reference hazards.
    #[inline(always)]
    pub fn set(&mut self, link: u32, length: u8) {
        unsafe {
            std::ptr::write_unaligned(std::ptr::addr_of_mut!(self.link), link);
            std::ptr::write_unaligned(std::ptr::addr_of_mut!(self.length), length);
        }
    }

    /// Sets the 32-bit link offset.
    #[inline(always)]
    pub fn set_link(&mut self, link: u32) {
        unsafe {
            std::ptr::write_unaligned(std::ptr::addr_of_mut!(self.link), link);
        }
    }

    /// Sets the 8-bit match length.
    #[inline(always)]
    pub fn set_length(&mut self, length: u8) {
        unsafe {
            std::ptr::write_unaligned(std::ptr::addr_of_mut!(self.length), length);
        }
    }
}

impl fmt::Debug for StructuredMatchEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StructuredMatchEntry")
            .field("link", &self.link())
            .field("length", &self.length())
            .finish()
    }
}

impl fmt::Display for StructuredMatchEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Structured(link={}, len={})", self.link(), self.length())
    }
}

/// Operational mode of the match table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchTableMode {
    /// 32-bit compact bit-packed mode (dict <= 64 MB, 4 bytes/entry).
    BitPacked,
    /// 40-bit structured packed mode (dict > 64 MB, 5 bytes/entry).
    Structured,
}

/// Dual-mode match table for LZMA2 match-finder pipelines.
///
/// Automatically selects optimal memory layout based on dictionary size:
/// - `<= 64 MB`: Allocates `BitPackedEntry` vector (4 bytes per entry, 100% 32-bit word aligned).
/// - `> 64 MB`: Allocates `StructuredMatchEntry` vector (5 bytes per entry, packed layout).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchTable {
    /// Bit-packed match table for standard dictionaries (<= 64 MB).
    BitPacked(Vec<BitPackedEntry>),
    /// Structured match table for extra-large dictionaries (> 64 MB up to 1 GB+).
    Structured(Vec<StructuredMatchEntry>),
}

impl MatchTable {
    /// Creates a new match table with automatic mode selection based on `dict_size`.
    pub fn new(dict_size: usize, num_entries: usize) -> Self {
        if dict_size <= COMPACT_DICT_THRESHOLD {
            Self::new_bit_packed(num_entries)
        } else {
            Self::new_structured(num_entries)
        }
    }

    /// Creates a new 32-bit `BitPacked` match table initialized with default entries.
    pub fn new_bit_packed(num_entries: usize) -> Self {
        Self::BitPacked(vec![BitPackedEntry::default(); num_entries])
    }

    /// Creates a new 40-bit `Structured` match table initialized with default entries.
    pub fn new_structured(num_entries: usize) -> Self {
        Self::Structured(vec![StructuredMatchEntry::default(); num_entries])
    }

    /// Returns the number of entries in the match table.
    #[inline(always)]
    pub fn len(&self) -> usize {
        match self {
            Self::BitPacked(entries) => entries.len(),
            Self::Structured(entries) => entries.len(),
        }
    }

    /// Returns `true` if the match table contains zero entries.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the active storage mode of the match table.
    #[inline(always)]
    pub fn mode(&self) -> MatchTableMode {
        match self {
            Self::BitPacked(_) => MatchTableMode::BitPacked,
            Self::Structured(_) => MatchTableMode::Structured,
        }
    }

    /// Returns the physical byte size occupied per entry (4 or 5 bytes).
    #[inline(always)]
    pub fn entry_size_bytes(&self) -> usize {
        match self {
            Self::BitPacked(_) => std::mem::size_of::<BitPackedEntry>(),
            Self::Structured(_) => std::mem::size_of::<StructuredMatchEntry>(),
        }
    }

    /// Returns the total physical memory in bytes consumed by the table entries.
    #[inline(always)]
    pub fn memory_bytes(&self) -> usize {
        self.len() * self.entry_size_bytes()
    }

    /// Retrieves the single raw entry at `pos` without jump-pointer chain expansion.
    ///
    /// Returns `Some((link, length))` if `pos` is in bounds, or `None` otherwise.
    #[inline]
    pub fn get_raw_entry(&self, pos: usize) -> Option<(u32, u32)> {
        match self {
            Self::BitPacked(entries) => entries.get(pos).map(|e| (e.link(), e.length())),
            Self::Structured(entries) => entries.get(pos).map(|e| (e.link(), e.length())),
        }
    }

    /// Retrieves the match at `pos` with automatic Jump-Pointer Chaining for extended lengths.
    ///
    /// If the recorded match length at `pos` equals the maximum single-entry capacity
    /// (63 for BitPacked, 255 for Structured), this method automatically follows the link chain
    /// forward, accumulating successive chunk lengths into a single total match length.
    ///
    /// Returns `(final_link, total_match_length)`. If `pos` is out of bounds, returns `(0, 0)`.
    pub fn get_match(&self, pos: usize) -> (u32, u32) {
        match self {
            Self::BitPacked(entries) => {
                if pos >= entries.len() {
                    return (0, 0);
                }
                let first = entries[pos];
                let first_len = first.length();
                let first_link = first.link();
                if first_len < BitPackedEntry::MAX_LENGTH {
                    return (first_link, first_len);
                }

                // Jump-Pointer Chaining: follow link chain to accumulate length
                let mut total_len = first_len;
                let mut current_link = first_link;
                let mut hops = 0;

                while hops < MAX_JUMP_CHAIN_HOPS {
                    let next_pos = current_link as usize;
                    if next_pos >= entries.len() || next_pos == 0 {
                        break;
                    }
                    let next_entry = entries[next_pos];
                    let next_len = next_entry.length();
                    let next_link = next_entry.link();

                    total_len = total_len.saturating_add(next_len);
                    current_link = next_link;
                    hops += 1;

                    if next_len < BitPackedEntry::MAX_LENGTH
                        || next_link == 0
                        || next_link as usize == next_pos
                    {
                        break;
                    }
                }

                (current_link, total_len)
            }
            Self::Structured(entries) => {
                if pos >= entries.len() {
                    return (0, 0);
                }
                let first = entries[pos];
                let first_len = first.length();
                let first_link = first.link();
                if first_len < StructuredMatchEntry::MAX_LENGTH {
                    return (first_link, first_len);
                }

                // Jump-Pointer Chaining: follow link chain to accumulate length
                let mut total_len = first_len;
                let mut current_link = first_link;
                let mut hops = 0;

                while hops < MAX_JUMP_CHAIN_HOPS {
                    let next_pos = current_link as usize;
                    if next_pos >= entries.len() || next_pos == 0 {
                        break;
                    }
                    let next_entry = entries[next_pos];
                    let next_len = next_entry.length();
                    let next_link = next_entry.link();

                    total_len = total_len.saturating_add(next_len);
                    current_link = next_link;
                    hops += 1;

                    if next_len < StructuredMatchEntry::MAX_LENGTH
                        || next_link == 0
                        || next_link as usize == next_pos
                    {
                        break;
                    }
                }

                (current_link, total_len)
            }
        }
    }

    /// Sets the match entry at `pos`.
    ///
    /// For `BitPacked`, values exceeding the 26/6-bit limits are masked.
    /// For `Structured`, lengths exceeding 255 are clamped to 255.
    #[inline]
    pub fn set_match(&mut self, pos: usize, link: u32, length: u32) {
        match self {
            Self::BitPacked(entries) => {
                if let Some(entry) = entries.get_mut(pos) {
                    entry.set(link, length);
                }
            }
            Self::Structured(entries) => {
                if let Some(entry) = entries.get_mut(pos) {
                    entry.set(link, length.min(StructuredMatchEntry::MAX_LENGTH) as u8);
                }
            }
        }
    }

    /// Resets all entries in the match table to default zero values.
    pub fn clear(&mut self) {
        match self {
            Self::BitPacked(entries) => {
                entries.fill(BitPackedEntry::default());
            }
            Self::Structured(entries) => {
                entries.fill(StructuredMatchEntry::default());
            }
        }
    }

    /// Returns a zero-copy mutable byte slice starting at entry `pos` through to the end of the table.
    ///
    /// Used by streaming compressor pipelines and hardware-accelerated match scanners
    /// to write match results directly into the underlying table buffer without intermediate copies.
    ///
    /// If `pos >= self.len()`, an empty mutable slice `&mut []` is returned.
    pub fn as_output_buffer_mut(&mut self, pos: usize) -> &mut [u8] {
        match self {
            Self::BitPacked(entries) => {
                if pos >= entries.len() {
                    return &mut [];
                }
                let remaining_entries = entries.len() - pos;
                let byte_count = remaining_entries * std::mem::size_of::<BitPackedEntry>();
                let ptr = entries[pos..].as_mut_ptr() as *mut u8;
                unsafe { std::slice::from_raw_parts_mut(ptr, byte_count) }
            }
            Self::Structured(entries) => {
                if pos >= entries.len() {
                    return &mut [];
                }
                let remaining_entries = entries.len() - pos;
                let byte_count = remaining_entries * std::mem::size_of::<StructuredMatchEntry>();
                let ptr = entries[pos..].as_mut_ptr() as *mut u8;
                unsafe { std::slice::from_raw_parts_mut(ptr, byte_count) }
            }
        }
    }

    /// Returns a zero-copy immutable byte slice representing the entire underlying buffer.
    pub fn as_byte_slice(&self) -> &[u8] {
        match self {
            Self::BitPacked(entries) => {
                let byte_count = entries.len() * std::mem::size_of::<BitPackedEntry>();
                let ptr = entries.as_ptr() as *const u8;
                unsafe { std::slice::from_raw_parts(ptr, byte_count) }
            }
            Self::Structured(entries) => {
                let byte_count = entries.len() * std::mem::size_of::<StructuredMatchEntry>();
                let ptr = entries.as_ptr() as *const u8;
                unsafe { std::slice::from_raw_parts(ptr, byte_count) }
            }
        }
    }

    /// Returns a zero-copy mutable byte slice representing the entire underlying buffer.
    pub fn as_byte_slice_mut(&mut self) -> &mut [u8] {
        self.as_output_buffer_mut(0)
    }
}
