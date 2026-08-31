// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! High-speed 14-bit / 15-bit hash table match finder for Google Snappy.
//!
//! Employs golden-ratio multiplicative hashing (`0x1e35a7bd`) with compact
//! 16-bit relative block offset buckets to maximize L1 data cache residency.

/// Maximum number of hash table address bits for standard 64KB Snappy block compression (14 bits = 16,384 buckets).
pub const SNAPPY_MAX_HASH_TABLE_BITS: usize = 14;

/// Extended 15-bit hash table address bits for large-window match finding (15 bits = 32,768 buckets).
pub const SNAPPY_MAX_HASH_TABLE_BITS_15: usize = 15;

/// Golden ratio multiplicative hash constant (`0x1e35a7bd`) from Google Snappy specification.
pub const SNAPPY_HASH_MAGIC: u32 = 0x1e35a7bd;

/// Number of bucket entries in a standard 14-bit Snappy hash table.
pub const SNAPPY_HASH_TABLE_SIZE_14: usize = 1 << SNAPPY_MAX_HASH_TABLE_BITS;

/// Number of bucket entries in an extended 15-bit Snappy hash table.
pub const SNAPPY_HASH_TABLE_SIZE_15: usize = 1 << SNAPPY_MAX_HASH_TABLE_BITS_15;

/// Minimum number of hash table address bits (8 bits = 256 buckets).
pub const SNAPPY_MIN_HASH_TABLE_BITS: usize = 8;

/// Computes golden-ratio multiplicative hash index from a 32-bit quad-byte integer.
///
/// Shifts the highest quality upper bits of the 32-bit product right by `shift`.
#[inline(always)]
pub fn hash_bytes(bytes: u32, shift: u32) -> usize {
    (bytes.wrapping_mul(SNAPPY_HASH_MAGIC) >> shift) as usize
}

/// Computes the optimal hash table bit-width based on the input fragment size.
#[inline]
pub fn calculate_table_bits(input_size: usize) -> usize {
    if input_size < (1 << SNAPPY_MIN_HASH_TABLE_BITS) {
        SNAPPY_MIN_HASH_TABLE_BITS
    } else {
        let mut bits = SNAPPY_MIN_HASH_TABLE_BITS;
        while bits < SNAPPY_MAX_HASH_TABLE_BITS && (1 << bits) < input_size {
            bits += 1;
        }
        bits
    }
}

/// Compact L1-cache-resident hash table storing 16-bit block relative offsets.
#[derive(Debug, Clone)]
pub struct SnappyHashTable {
    /// 16-bit relative offset table entries.
    entries: Box<[u16]>,
    /// Shift count applied to 32-bit hash multiplication (`32 - bits`).
    shift: u32,
    /// Bitmask for table indexing (`capacity - 1`).
    mask: usize,
}

impl Default for SnappyHashTable {
    fn default() -> Self {
        Self::new()
    }
}

impl SnappyHashTable {
    /// Creates a new default 14-bit Snappy hash table (16,384 entries / 32 KB).
    pub fn new() -> Self {
        Self::with_bits(SNAPPY_MAX_HASH_TABLE_BITS)
    }

    /// Creates a hash table with a specified number of address bits (`8..=15`).
    pub fn with_bits(bits: usize) -> Self {
        let clamped_bits = bits.clamp(SNAPPY_MIN_HASH_TABLE_BITS, SNAPPY_MAX_HASH_TABLE_BITS_15);
        let capacity = 1 << clamped_bits;
        let shift = (32 - clamped_bits) as u32;
        let mask = capacity - 1;
        Self {
            entries: vec![0u16; capacity].into_boxed_slice(),
            shift,
            mask,
        }
    }

    /// Clears all entries in the hash table by resetting them to zero.
    #[inline]
    pub fn clear(&mut self) {
        self.entries.fill(0);
    }

    /// Returns the number of bucket entries in the hash table.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.entries.len()
    }

    /// Returns the shift amount used for multiplicative hashing.
    #[inline]
    pub fn shift(&self) -> u32 {
        self.shift
    }

    /// Computes the bucket table index for a 32-bit integer word.
    #[inline(always)]
    pub fn hash(&self, dword: u32) -> usize {
        hash_bytes(dword, self.shift) & self.mask
    }

    /// Looks up the previous candidate position and updates the bucket with `current_pos`.
    ///
    /// Returns the previous relative offset stored at the bucket.
    #[inline(always)]
    pub fn lookup_and_update(&mut self, dword: u32, current_pos: usize) -> usize {
        let idx = self.hash(dword);
        let prev = self.entries[idx] as usize;
        self.entries[idx] = current_pos as u16;
        prev
    }

    /// Updates the hash table bucket for `dword` with `current_pos` without returning the previous value.
    #[inline(always)]
    pub fn update(&mut self, dword: u32, current_pos: usize) {
        let idx = self.hash(dword);
        self.entries[idx] = current_pos as u16;
    }

    /// Looks up the existing candidate position for `dword` without updating the bucket.
    #[inline(always)]
    pub fn lookup(&self, dword: u32) -> usize {
        let idx = self.hash(dword);
        self.entries[idx] as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snappy_hash_magic_constant() {
        assert_eq!(SNAPPY_HASH_MAGIC, 0x1e35a7bd);
        assert_eq!(SNAPPY_MAX_HASH_TABLE_BITS, 14);
        assert_eq!(SNAPPY_MAX_HASH_TABLE_BITS_15, 15);
        assert_eq!(SNAPPY_HASH_TABLE_SIZE_14, 16384);
        assert_eq!(SNAPPY_HASH_TABLE_SIZE_15, 32768);
    }

    #[test]
    fn test_hash_bytes_distribution() {
        let shift = (32 - SNAPPY_MAX_HASH_TABLE_BITS) as u32;
        let h1 = hash_bytes(0x12345678, shift);
        let h2 = hash_bytes(0x12345679, shift);
        assert_ne!(h1, h2);
        assert!(h1 < SNAPPY_HASH_TABLE_SIZE_14);
        assert!(h2 < SNAPPY_HASH_TABLE_SIZE_14);
    }

    #[test]
    fn test_hash_table_lookup_and_update_lifecycle() {
        let mut table = SnappyHashTable::new();
        assert_eq!(table.capacity(), 16384);

        let dword = 0xDEADBEEF;
        let initial_prev = table.lookup_and_update(dword, 100);
        assert_eq!(initial_prev, 0);

        let second_prev = table.lookup_and_update(dword, 250);
        assert_eq!(second_prev, 100);

        let current = table.lookup(dword);
        assert_eq!(current, 250);

        table.clear();
        assert_eq!(table.lookup(dword), 0);
    }

    #[test]
    fn test_hash_table_15_bit_instantiation() {
        let mut table = SnappyHashTable::with_bits(15);
        assert_eq!(table.capacity(), 32768);
        assert_eq!(table.shift(), 17);

        let dword = 0x01020304;
        table.update(dword, 1024);
        assert_eq!(table.lookup(dword), 1024);
    }

    #[test]
    fn test_calculate_table_bits() {
        assert_eq!(calculate_table_bits(100), 8);
        assert_eq!(calculate_table_bits(256), 8);
        assert_eq!(calculate_table_bits(512), 9);
        assert_eq!(calculate_table_bits(65536), 14);
        assert_eq!(calculate_table_bits(1000000), 14);
    }
}
