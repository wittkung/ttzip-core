// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use std::collections::HashMap;

/// A high-performance contiguous String Arena with bidirectional deduplication.
/// Generates a flat byte buffer for the `TT_SECTION_STRING_ARENA` snapshot section.
/// Uses a zero-allocation deduplication map by borrowing string slices from the input AST.
pub struct StringArena<'a> {
    buffer: Vec<u8>,
    dedup: HashMap<&'a [u8], u32>,
}

impl<'a> Default for StringArena<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> StringArena<'a> {
    /// Creates a new StringArena with a pre-allocated capacity to minimize reallocations.
    pub fn new() -> Self {
        Self::with_capacity(1024 * 1024, 128 * 1024)
    }

    /// Creates a new StringArena with explicit capacity for the buffer and deduplication map.
    pub fn with_capacity(buffer_capacity: usize, map_capacity: usize) -> Self {
        // We always start the buffer with a single null byte.
        // This ensures that an offset of 0 maps to an empty string.
        let mut buffer = Vec::with_capacity(buffer_capacity);
        buffer.push(0);

        Self {
            buffer,
            dedup: HashMap::with_capacity(map_capacity),
        }
    }

    /// Interns an arbitrary byte slice into the arena, supporting non-UTF8 paths.
    /// If the slice is already present, returns its existing byte offset.
    /// Otherwise, appends the slice (and a null terminator) and returns the new offset.
    pub fn intern_bytes(&mut self, bytes: &'a [u8]) -> u32 {
        if bytes.is_empty() {
            return 0;
        }

        if let Some(&offset) = self.dedup.get(bytes) {
            return offset;
        }

        let offset = self.buffer.len() as u32;
        self.buffer.extend_from_slice(bytes);
        self.buffer.push(0); // Null terminator

        self.dedup.insert(bytes, offset);
        offset
    }

    /// Interns a string slice into the arena.
    /// Forwards directly to `intern_bytes(s.as_bytes())`.
    #[inline]
    pub fn intern(&mut self, s: &'a str) -> u32 {
        self.intern_bytes(s.as_bytes())
    }

    /// Consumes the arena, returning the raw byte buffer.
    /// The returned buffer is guaranteed to be 8-byte aligned if reallocated by Mach,
    /// but the Vec itself is standard heap-allocated.
    pub fn into_bytes(self) -> Vec<u8> {
        self.buffer
    }

    /// Returns the current total size of the string arena in bytes.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Returns true if the string arena is empty (contains only the initial null byte).
    pub fn is_empty(&self) -> bool {
        self.buffer.len() <= 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_arena_intern_and_dedup() {
        let mut arena = StringArena::new();
        assert_eq!(arena.intern(""), 0);
        assert_eq!(arena.intern_bytes(b""), 0);

        let off_hello1 = arena.intern("hello");
        assert_eq!(off_hello1, 1);
        let off_hello2 = arena.intern("hello");
        assert_eq!(off_hello2, off_hello1);

        let off_hello_bytes = arena.intern_bytes(b"hello");
        assert_eq!(off_hello_bytes, off_hello1);

        let off_world = arena.intern("world");
        assert_ne!(off_world, off_hello1);

        // Test non-UTF8 arbitrary byte sequences
        let non_utf8 = b"\xFF\xFE\xFD\xFC";
        let off_non_utf8_1 = arena.intern_bytes(non_utf8);
        let off_non_utf8_2 = arena.intern_bytes(non_utf8);
        assert_eq!(off_non_utf8_1, off_non_utf8_2);

        let bytes = arena.into_bytes();
        assert_eq!(&bytes[off_hello1 as usize..off_hello1 as usize + 5], b"hello");
        assert_eq!(&bytes[off_non_utf8_1 as usize..off_non_utf8_1 as usize + 4], non_utf8);
    }
}
