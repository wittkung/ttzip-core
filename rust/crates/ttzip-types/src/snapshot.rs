// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Self-describing 128-byte snapshot header and 32-byte section entry table.
//!
//! # Microarchitecture & Darwin Mach VM Invariants
//! - Header Size: Exactly 128 bytes, 64-byte cache-line aligned (`#[repr(C, align(64))]`).
//! - Section Entry Size: Exactly 32 bytes, 8-byte aligned (`#[repr(C, align(8))]`).
//! - Mach VM Page Alignment: `total_mapping_size` MUST be an exact multiple of 16,384 bytes
//!   (Apple Silicon 16KB hardware VM page size) to support zero-copy COW Mach port sharing.
//! - Section Table: `section_headers_offset` must be 64-byte aligned and point to a contiguous
//!   array of `TTWorkspaceSectionEntry`.

use core::fmt;

pub const TT_SNAPSHOT_MAGIC: u32 = 0x5356_5454; // "TTVS" Little-Endian
pub const TT_ABI_VERSION_MAJOR: u16 = 6;
pub const TT_ABI_VERSION_MINOR: u16 = 0;
pub const TT_HEADER_SIZE: u32 = 128;
pub const TT_SECTION_HEADER_SIZE: u32 = 32;
pub const MACH_VM_PAGE_SIZE_16KB: usize = 16384;

// --- Section Types (C-ABI v6.0 Standard Section IDs) ---
pub const TT_SECTION_TYPE_UNUSED: u32 = 0;
pub const TT_SECTION_TYPE_NODES: u32 = 1;
pub const TT_SECTION_TYPE_STRING_ARENA: u32 = 2;
pub const TT_SECTION_TYPE_STABLE_ID_KEYS: u32 = 3;
pub const TT_SECTION_TYPE_STABLE_ID_INDICES: u32 = 4;
pub const TT_SECTION_TYPE_DEVICE_ID_KEYS: u32 = 5;
pub const TT_SECTION_TYPE_DEVICE_ID_INDICES: u32 = 6;
pub const TT_SECTION_TYPE_CUSTOM_METADATA: u32 = 7;
pub const TT_SECTION_TYPE_NAME_MASKS: u32 = 8;
pub const TT_SECTION_TYPE_EMBEDDINGS: u32 = 9;

// Backward compatibility alias for legacy subtree end IDs
pub const TT_SECTION_TYPE_SUBTREE_END_IDS: u32 = 9;

// Backward compatibility aliases mapping to canonical TT_SECTION_TYPE_*
pub const TT_SECTION_VFS_NODES: u32 = TT_SECTION_TYPE_NODES;
pub const TT_SECTION_STRING_ARENA: u32 = TT_SECTION_TYPE_STRING_ARENA;

// --- Embedding Vector Constants ---
pub const TT_EMBEDDING_MAGIC: u32 = 0x5454_5645; // "TTVE" Little-Endian
pub const TT_EMBEDDING_VERSION_1: u32 = 1;
pub const TT_EMBEDDING_DIMENSION_128: u16 = 128;
pub const TT_EMBEDDING_QUANTIZATION_INT8: u8 = 1;

/// Self-describing section type tag (1..9) aligning with C-ABI v6.0 contracts.
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TTWorkspaceSectionType {
    Unused = 0,
    Nodes = 1,
    StringArena = 2,
    StableIdKeys = 3,
    StableIdIndices = 4,
    DeviceIdKeys = 5,
    DeviceIdIndices = 6,
    CustomMetadata = 7,
    NameMasks = 8,
    Embeddings = 9,
}

impl TTWorkspaceSectionType {
    pub const TT_SECTION_TYPE_NAME_MASKS: u32 = 8;
    pub const TT_SECTION_TYPE_EMBEDDINGS: u32 = 9;
    pub const TT_SECTION_TYPE_SUBTREE_END_IDS: u32 = 9;

    #[inline]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }
}

// --- Section Flags ---
pub const TT_SECTION_FLAG_ALIGNED_64: u32 = 1 << 0;  // 0x0001: Section payload requires 64-byte alignment
pub const TT_SECTION_FLAG_READ_ONLY: u32 = 1 << 1;   // 0x0002: Section payload is write-protected
pub const TT_SECTION_FLAG_COMPRESSED: u32 = 1 << 2;  // 0x0004: Section payload is LZ4/ZSTD compressed
pub const TT_SECTION_FLAG_ENCRYPTED: u32 = 1 << 3;   // 0x0008: Section payload is encrypted
pub const TT_SECTION_FLAG_MACH_SHARED: u32 = 1 << 4; // 0x0010: Section is Mach VM copy-on-write shared

// --- Feature Flags ---
pub const TT_FEATURE_FLAG_CSR: u32 = 1 << 0;
pub const TT_FEATURE_FLAG_LEXSORT: u32 = 1 << 1;
pub const TT_FEATURE_FLAG_DFS_BLOCK: u32 = 1 << 2;
pub const TT_FEATURE_FLAG_NAME_MASK: u32 = 1 << 3;

/// 32-byte Section Table Entry.
#[repr(C, align(8))]
#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub struct TTWorkspaceSectionEntry {
    pub section_type: u32,  // 0..4:   TT_SECTION_* type ID
    pub flags: u32,         // 4..8:   TT_SECTION_FLAG_* bitmask
    pub offset: u64,        // 8..16:  Byte offset from base of snapshot mapping
    pub size_bytes: u64,    // 16..24: Total payload size in bytes
    pub entry_count: u64,   // 24..32: Logical element count (e.g. node_count)
}

/// 128-byte Self-Describing Snapshot Header.
#[repr(C, align(64))]
#[derive(Copy, Clone)]
pub struct TTWorkspaceSnapshotHeader {
    pub magic: u32,                    // 0..4:   0x53565454 "TTVS" (Little-Endian)
    pub abi_major: u16,                // 4..6:   0x0006 (v6.0 Section Header Table)
    pub abi_minor: u16,                // 6..8:   0x0000
    pub header_size: u32,              // 8..12:  Snapshot header size in bytes (128)
    pub section_header_size: u32,      // 12..16: Size of each section header entry (32)
    pub section_count: u32,            // 16..20: Number of active sections in section table
    pub root_node_id: u32,             // 20..24: Root node index (strictly 0)
    pub total_mapping_size: u64,       // 24..32: Total mapped Mach VM bytes (multiple of 16384)
    pub generation: u64,               // 32..40: Monotonic snapshot generation (RCU)
    pub snapshot_sequence: u64,        // 40..48: Monotonic snapshot sequence counter
    pub section_headers_offset: u64,   // 48..56: Byte offset to TTWorkspaceSectionEntry[] (64B aligned)
    pub feature_flags: u32,            // 56..60: Feature flags (Bit 0: CSR, Bit 1: LexSort)
    pub reserved_flags: u32,           // 60..64: Reserved for future feature expansion
    pub snapshot_checksum: u64,        // 64..72: Reserved zero
    pub _reserved: [u8; 56],           // 72..128: Generous 56-byte reservation
}

/// 32-byte Section Header for Vector Embeddings, 64-byte microarchitecture aligned.
#[repr(C, align(64))]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct EmbeddingSectionHeader {
    /// Magic identifier: 0x54545645 ("TTVE" = TTZip Vector Embeddings)
    pub magic: u32,
    /// Schema format version: 1
    pub version: u32,
    /// Total number of vector records in this section (must match total_nodes)
    pub entry_count: u32,
    /// Dimensionality of each vector: 128
    pub dimension: u16,
    /// Quantization format: 1 = Signed 8-bit Integer (i8)
    pub quantization: u8,
    /// Reserved alignment padding
    pub _reserved1: u8,
    /// Hash of the base model weights (xxHash3 64-bit seed)
    pub model_fingerprint: u64,
    /// Reserved for future expansion
    pub _reserved2: [u8; 8],
}

impl Default for EmbeddingSectionHeader {
    #[inline]
    fn default() -> Self {
        Self {
            magic: TT_EMBEDDING_MAGIC,
            version: TT_EMBEDDING_VERSION_1,
            entry_count: 0,
            dimension: TT_EMBEDDING_DIMENSION_128,
            quantization: TT_EMBEDDING_QUANTIZATION_INT8,
            _reserved1: 0,
            model_fingerprint: 0,
            _reserved2: [0u8; 8],
        }
    }
}

impl fmt::Debug for EmbeddingSectionHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EmbeddingSectionHeader")
            .field("magic", &format_args!("0x{:08x}", self.magic))
            .field("version", &self.version)
            .field("entry_count", &self.entry_count)
            .field("dimension", &self.dimension)
            .field("quantization", &self.quantization)
            .field("model_fingerprint", &format_args!("0x{:016x}", self.model_fingerprint))
            .finish()
    }
}

impl EmbeddingSectionHeader {
    /// Validates magic, version, dimensionality, and quantization format.
    #[inline]
    pub fn validate(&self) -> bool {
        self.magic == TT_EMBEDDING_MAGIC
            && self.version == TT_EMBEDDING_VERSION_1
            && self.dimension == TT_EMBEDDING_DIMENSION_128
            && self.quantization == TT_EMBEDDING_QUANTIZATION_INT8
    }
}

/// 128-byte signed 8-bit quantized embedding vector, 64-byte aligned for direct ARM NEON streaming.
#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompactVector128 {
    pub data: [i8; 128],
}

impl CompactVector128 {
    #[inline(always)]
    pub const fn zero() -> Self {
        Self { data: [0i8; 128] }
    }

    #[inline(always)]
    pub const fn new(data: [i8; 128]) -> Self {
        Self { data }
    }
}

impl Default for CompactVector128 {
    #[inline(always)]
    fn default() -> Self {
        Self::zero()
    }
}

impl AsRef<[i8; 128]> for CompactVector128 {
    #[inline(always)]
    fn as_ref(&self) -> &[i8; 128] {
        &self.data
    }
}

impl AsMut<[i8; 128]> for CompactVector128 {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [i8; 128] {
        &mut self.data
    }
}

impl core::ops::Deref for CompactVector128 {
    type Target = [i8; 128];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl core::ops::DerefMut for CompactVector128 {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl From<[i8; 128]> for CompactVector128 {
    #[inline(always)]
    fn from(data: [i8; 128]) -> Self {
        Self { data }
    }
}

// Compile-time static assertions ensuring exact geometry
const _: () = {
    assert!(core::mem::size_of::<TTWorkspaceSectionEntry>() == 32);
    assert!(core::mem::align_of::<TTWorkspaceSectionEntry>() == 8);
    assert!(core::mem::size_of::<TTWorkspaceSnapshotHeader>() == 128);
    assert!(core::mem::align_of::<TTWorkspaceSnapshotHeader>() == 64);
    assert!(core::mem::size_of::<EmbeddingSectionHeader>() == 64);
    assert!(core::mem::align_of::<EmbeddingSectionHeader>() == 64);
    assert!(core::mem::size_of::<CompactVector128>() == 128);
    assert!(core::mem::align_of::<CompactVector128>() == 64);
};

impl Default for TTWorkspaceSnapshotHeader {
    fn default() -> Self {
        Self {
            magic: TT_SNAPSHOT_MAGIC,
            abi_major: TT_ABI_VERSION_MAJOR,
            abi_minor: TT_ABI_VERSION_MINOR,
            header_size: TT_HEADER_SIZE,
            section_header_size: TT_SECTION_HEADER_SIZE,
            section_count: 0,
            root_node_id: 0,
            total_mapping_size: 0,
            generation: 1,
            snapshot_sequence: 1,
            section_headers_offset: 128,
            feature_flags: 0x0000_0003, // CSR + LexSort
            reserved_flags: 0,
            snapshot_checksum: 0,
            _reserved: [0u8; 56],
        }
    }
}

impl fmt::Debug for TTWorkspaceSectionEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TTWorkspaceSectionEntry")
            .field("section_type", &self.section_type)
            .field("flags", &format_args!("0x{:08x}", self.flags))
            .field("offset", &self.offset)
            .field("size_bytes", &self.size_bytes)
            .field("entry_count", &self.entry_count)
            .finish()
    }
}

impl fmt::Debug for TTWorkspaceSnapshotHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TTWorkspaceSnapshotHeader")
            .field("magic", &format_args!("0x{:08x}", self.magic))
            .field("abi_version", &format_args!("{}.{}", self.abi_major, self.abi_minor))
            .field("header_size", &self.header_size)
            .field("section_count", &self.section_count)
            .field("root_node_id", &self.root_node_id)
            .field("total_mapping_size", &self.total_mapping_size)
            .field("generation", &self.generation)
            .field("sequence", &self.snapshot_sequence)
            .field("section_headers_offset", &self.section_headers_offset)
            .field("feature_flags", &format_args!("0x{:08x}", self.feature_flags))
            .finish()
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SnapshotHeaderError {
    InvalidMagic(u32),
    UnsupportedAbiMajor(u16),
    InvalidHeaderSize(u32),
    InvalidSectionHeaderSize(u32),
    UnalignedMappingSize(u64),
    UnalignedSectionOffset(u64),
    SectionTableOutOfBounds,
    BufferTooSmall { required: usize, actual: usize },
}

impl TTWorkspaceSnapshotHeader {
    /// Validate self-consistency and memory boundary constraints against physical mapping size.
    pub fn validate(&self, actual_buffer_len: usize) -> Result<(), SnapshotHeaderError> {
        if self.magic != TT_SNAPSHOT_MAGIC {
            return Err(SnapshotHeaderError::InvalidMagic(self.magic));
        }
        if self.abi_major != TT_ABI_VERSION_MAJOR {
            return Err(SnapshotHeaderError::UnsupportedAbiMajor(self.abi_major));
        }
        if self.header_size != TT_HEADER_SIZE {
            return Err(SnapshotHeaderError::InvalidHeaderSize(self.header_size));
        }
        if self.section_header_size != TT_SECTION_HEADER_SIZE {
            return Err(SnapshotHeaderError::InvalidSectionHeaderSize(self.section_header_size));
        }
        // Apple Silicon 16KB Page alignment requirement
        if !self.total_mapping_size.is_multiple_of(MACH_VM_PAGE_SIZE_16KB as u64) {
            return Err(SnapshotHeaderError::UnalignedMappingSize(self.total_mapping_size));
        }
        if actual_buffer_len < self.total_mapping_size as usize {
            return Err(SnapshotHeaderError::BufferTooSmall {
                required: self.total_mapping_size as usize,
                actual: actual_buffer_len,
            });
        }
        if !self.section_headers_offset.is_multiple_of(8) {
            return Err(SnapshotHeaderError::UnalignedSectionOffset(self.section_headers_offset));
        }

        let section_table_bytes = (self.section_count as usize)
            .checked_mul(self.section_header_size as usize)
            .ok_or(SnapshotHeaderError::SectionTableOutOfBounds)?;
        let section_table_end = (self.section_headers_offset as usize)
            .checked_add(section_table_bytes)
            .ok_or(SnapshotHeaderError::SectionTableOutOfBounds)?;

        if section_table_end > self.total_mapping_size as usize {
            return Err(SnapshotHeaderError::SectionTableOutOfBounds);
        }

        Ok(())
    }

    /// Locate a section descriptor by section type in the raw memory mapping.
    pub fn find_section<'a>(
        &self,
        mapping: &'a [u8],
        target_section_type: u32,
    ) -> Option<&'a TTWorkspaceSectionEntry> {
        if self.section_count == 0 || self.section_headers_offset == 0 {
            return None;
        }

        let start = self.section_headers_offset as usize;
        let count = self.section_count as usize;
        let entry_size = core::mem::size_of::<TTWorkspaceSectionEntry>();
        let total_size = count.checked_mul(entry_size)?;
        let end = start.checked_add(total_size)?;

        let slice = mapping.get(start..end)?;
        // Safety: Pointer is within slice bounds and TTWorkspaceSectionEntry alignment is 8 bytes.
        // section_headers_offset is verified to be 8-byte aligned.
        if !(slice.as_ptr() as usize).is_multiple_of(8) {
            return None;
        }

        let entries = unsafe {
            core::slice::from_raw_parts(slice.as_ptr() as *const TTWorkspaceSectionEntry, count)
        };

        entries.iter().find(|e| e.section_type == target_section_type)
    }

    /// Extract the payload byte slice for a specific section.
    pub fn get_section_payload<'a>(
        &self,
        mapping: &'a [u8],
        target_section_type: u32,
    ) -> Option<&'a [u8]> {
        let entry = self.find_section(mapping, target_section_type)?;
        let start = entry.offset as usize;
        let end = start.checked_add(entry.size_bytes as usize)?;
        mapping.get(start..end)
    }

    /// Extracts the typed embedding section header and slice of vectors from the snapshot mapping.
    pub fn find_embeddings<'a>(
        &self,
        mapping: &'a [u8],
    ) -> Option<(&'a EmbeddingSectionHeader, &'a [CompactVector128])> {
        let entry = self.find_section(mapping, TT_SECTION_TYPE_EMBEDDINGS)?;
        let start = entry.offset as usize;
        let size = entry.size_bytes as usize;
        let payload = mapping.get(start..start.checked_add(size)?)?;

        if !(payload.as_ptr() as usize).is_multiple_of(64) {
            return None;
        }

        let header_size = core::mem::size_of::<EmbeddingSectionHeader>();
        if payload.len() < header_size {
            return None;
        }

        let header = unsafe { &*(payload.as_ptr() as *const EmbeddingSectionHeader) };
        if !header.validate() {
            return None;
        }

        let vector_count = header.entry_count as usize;
        let expected_vector_bytes = vector_count.checked_mul(core::mem::size_of::<CompactVector128>())?;
        let total_required = header_size.checked_add(expected_vector_bytes)?;
        if payload.len() < total_required {
            return None;
        }

        let vectors_ptr = unsafe { payload.as_ptr().add(header_size) as *const CompactVector128 };
        let vectors = unsafe { core::slice::from_raw_parts(vectors_ptr, vector_count) };

        Some((header, vectors))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_header_geometry() {
        assert_eq!(core::mem::size_of::<TTWorkspaceSnapshotHeader>(), 128);
        assert_eq!(core::mem::align_of::<TTWorkspaceSnapshotHeader>(), 64);
        assert_eq!(core::mem::size_of::<TTWorkspaceSectionEntry>(), 32);
        assert_eq!(core::mem::align_of::<TTWorkspaceSectionEntry>(), 8);
        assert_eq!(core::mem::size_of::<EmbeddingSectionHeader>(), 64);
        assert_eq!(core::mem::align_of::<EmbeddingSectionHeader>(), 64);
        assert_eq!(core::mem::size_of::<CompactVector128>(), 128);
        assert_eq!(core::mem::align_of::<CompactVector128>(), 64);
    }

    #[test]
    fn test_embedding_header_and_compact_vector() {
        let mut hdr = EmbeddingSectionHeader::default();
        assert_eq!(hdr.magic, TT_EMBEDDING_MAGIC);
        assert_eq!(hdr.version, 1);
        assert_eq!(hdr.dimension, 128);
        assert_eq!(hdr.quantization, 1);
        assert!(hdr.validate());

        hdr.magic = 0x12345678;
        assert!(!hdr.validate());

        let mut v = CompactVector128::zero();
        assert_eq!(v.data[0], 0);
        v.data[127] = 42;
        assert_eq!(v[127], 42);
        v[0] = -128;
        assert_eq!(v.as_ref()[0], -128);
    }

    #[test]
    fn test_snapshot_header_validation_and_lookup() {
        let total_size = 16384; // 1 x 16KB page
        let mut buffer = vec![0u8; total_size];

        let header = TTWorkspaceSnapshotHeader {
            total_mapping_size: total_size as u64,
            section_count: 2,
            section_headers_offset: 128,
            ..TTWorkspaceSnapshotHeader::default()
        };

        // Place section entries right after header
        let entries = [
            TTWorkspaceSectionEntry {
                section_type: TT_SECTION_TYPE_NODES,
                flags: TT_SECTION_FLAG_ALIGNED_64 | TT_SECTION_FLAG_READ_ONLY,
                offset: 512,
                size_bytes: 1024,
                entry_count: 16,
            },
            TTWorkspaceSectionEntry {
                section_type: TT_SECTION_TYPE_STRING_ARENA,
                flags: TT_SECTION_FLAG_ALIGNED_64 | TT_SECTION_FLAG_READ_ONLY,
                offset: 2048,
                size_bytes: 4096,
                entry_count: 0,
            },
        ];

        // Write header to buffer
        unsafe {
            let hdr_ptr = buffer.as_mut_ptr() as *mut TTWorkspaceSnapshotHeader;
            core::ptr::write(hdr_ptr, header);

            let sec_ptr = buffer.as_mut_ptr().add(128) as *mut TTWorkspaceSectionEntry;
            core::ptr::copy_nonoverlapping(entries.as_ptr(), sec_ptr, 2);
        }

        let parsed_header = unsafe { &*(buffer.as_ptr() as *const TTWorkspaceSnapshotHeader) };
        assert!(parsed_header.validate(buffer.len()).is_ok());

        // Find existing sections
        let vfs_sec = parsed_header.find_section(&buffer, TT_SECTION_TYPE_NODES);
        assert!(vfs_sec.is_some());
        assert_eq!(vfs_sec.unwrap().offset, 512);

        let str_sec = parsed_header.find_section(&buffer, TT_SECTION_TYPE_STRING_ARENA);
        assert!(str_sec.is_some());
        assert_eq!(str_sec.unwrap().size_bytes, 4096);

        // Find non-existing section
        let missing = parsed_header.find_section(&buffer, TT_SECTION_TYPE_CUSTOM_METADATA);
        assert!(missing.is_none());

        // Test unaligned total_mapping_size error
        let mut bad_header = header;
        bad_header.total_mapping_size = 16380; // Not 16KB aligned
        assert_eq!(
            bad_header.validate(buffer.len()),
            Err(SnapshotHeaderError::UnalignedMappingSize(16380))
        );
    }

    #[test]
    fn test_section_types_and_feature_flags() {
        assert_eq!(TTWorkspaceSectionType::NameMasks as u32, 8);
        assert_eq!(TTWorkspaceSectionType::Embeddings as u32, 9);
        assert_eq!(TT_SECTION_TYPE_UNUSED, 0);
        assert_eq!(TT_SECTION_TYPE_NODES, 1);
        assert_eq!(TT_SECTION_TYPE_STRING_ARENA, 2);
        assert_eq!(TT_SECTION_TYPE_STABLE_ID_KEYS, 3);
        assert_eq!(TT_SECTION_TYPE_STABLE_ID_INDICES, 4);
        assert_eq!(TT_SECTION_TYPE_DEVICE_ID_KEYS, 5);
        assert_eq!(TT_SECTION_TYPE_DEVICE_ID_INDICES, 6);
        assert_eq!(TT_SECTION_TYPE_CUSTOM_METADATA, 7);
        assert_eq!(TT_SECTION_TYPE_NAME_MASKS, 8);
        assert_eq!(TT_SECTION_TYPE_EMBEDDINGS, 9);
        assert_eq!(TT_SECTION_TYPE_SUBTREE_END_IDS, 9);
        assert_eq!(TT_SECTION_VFS_NODES, 1);
        assert_eq!(TT_SECTION_STRING_ARENA, 2);
        assert_eq!(TTWorkspaceSectionType::TT_SECTION_TYPE_NAME_MASKS, 8);
        assert_eq!(TTWorkspaceSectionType::TT_SECTION_TYPE_EMBEDDINGS, 9);
        assert_eq!(TTWorkspaceSectionType::TT_SECTION_TYPE_SUBTREE_END_IDS, 9);
        assert_eq!(TTWorkspaceSectionType::NameMasks.as_u32(), 8);
        assert_eq!(TTWorkspaceSectionType::Embeddings.as_u32(), 9);

        assert_eq!(TT_SECTION_FLAG_ALIGNED_64, 1 << 0);
        assert_eq!(TT_SECTION_FLAG_READ_ONLY, 1 << 1);
        assert_eq!(TT_SECTION_FLAG_COMPRESSED, 1 << 2);
        assert_eq!(TT_SECTION_FLAG_ENCRYPTED, 1 << 3);
        assert_eq!(TT_SECTION_FLAG_MACH_SHARED, 1 << 4);

        assert_eq!(TT_FEATURE_FLAG_CSR, 1 << 0);
        assert_eq!(TT_FEATURE_FLAG_LEXSORT, 1 << 1);
        assert_eq!(TT_FEATURE_FLAG_DFS_BLOCK, 1 << 2);
        assert_eq!(TT_FEATURE_FLAG_NAME_MASK, 1 << 3);
    }
}
