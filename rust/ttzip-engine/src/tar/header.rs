// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! POSIX ustar, GNU TAR, and V7 512-byte header sector geometry memory mapping.

use std::fmt;
use std::ops::{Deref, DerefMut};

use super::codec::{
    null_trimmed_str, numeric_extended_from, numeric_extended_into, octal_from, octal_into,
};
pub use super::codec::{base256_from, base256_into};
pub use super::types::{
    GnuExtSparseHeader, GnuHeader, GnuSparseHeader, OldHeader, TarEntryType, UstarHeader,
    BLOCK_SIZE, LEN_CHKSUM, LEN_DEVMAJOR, LEN_DEVMINOR, LEN_GID, LEN_GNAME, LEN_LINKNAME,
    LEN_MAGIC, LEN_MODE, LEN_MTIME, LEN_NAME, LEN_PREFIX, LEN_SIZE, LEN_TYPEFLAG, LEN_UID,
    LEN_UNAME, LEN_VERSION, MAGIC_GNU, MAGIC_USTAR, OFFSET_CHKSUM, OFFSET_DEVMAJOR,
    OFFSET_DEVMINOR, OFFSET_GID, OFFSET_GNAME, OFFSET_LINKNAME, OFFSET_MAGIC, OFFSET_MODE,
    OFFSET_MTIME, OFFSET_NAME, OFFSET_PREFIX, OFFSET_SIZE, OFFSET_TYPEFLAG, OFFSET_UID,
    OFFSET_UNAME, OFFSET_VERSION, VERSION_GNU, VERSION_USTAR,
};

/// Hardware-aligned 512-byte TAR sector memory wrapper (`#[repr(C, align(512))]`).
#[repr(C, align(512))]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct TarHeader {
    /// Contiguous 512-byte raw sector data.
    pub bytes: [u8; BLOCK_SIZE],
}

impl Default for TarHeader {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for TarHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TarHeader")
            .field("name", &self.name())
            .field("mode", &format_args!("{:#o}", self.mode()))
            .field("uid", &self.uid())
            .field("gid", &self.gid())
            .field("size", &self.size())
            .field("mtime", &self.mtime())
            .field("chksum", &self.chksum())
            .field("typeflag", &self.entry_type())
            .field("linkname", &self.linkname())
            .field("magic", &String::from_utf8_lossy(self.magic_bytes()))
            .field("version", &String::from_utf8_lossy(self.version_bytes()))
            .field("uname", &self.uname())
            .field("gname", &self.gname())
            .field("devmajor", &self.devmajor())
            .field("devminor", &self.devminor())
            .field("prefix", &self.prefix())
            .finish()
    }
}

impl Deref for TarHeader {
    type Target = [u8; BLOCK_SIZE];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

impl DerefMut for TarHeader {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.bytes
    }
}

impl AsRef<[u8]> for TarHeader {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

impl AsMut<[u8]> for TarHeader {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }
}

impl TarHeader {
    /// Creates a zeroed 512-byte aligned TAR header block.
    #[inline]
    pub const fn new() -> Self {
        Self {
            bytes: [0u8; BLOCK_SIZE],
        }
    }

    /// Wraps an existing 512-byte array into an aligned `TarHeader`.
    #[inline]
    pub const fn from_bytes(bytes: [u8; BLOCK_SIZE]) -> Self {
        Self { bytes }
    }

    /// Constructs a `TarHeader` by copying from a slice of length at least 512 bytes.
    #[inline]
    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        if slice.len() < BLOCK_SIZE {
            return None;
        }
        let mut header = Self::new();
        header.bytes.copy_from_slice(&slice[..BLOCK_SIZE]);
        Some(header)
    }

    /// Returns a reference to the inner 512-byte array.
    #[inline]
    pub const fn as_bytes(&self) -> &[u8; BLOCK_SIZE] {
        &self.bytes
    }

    /// Returns a mutable reference to the inner 512-byte array.
    #[inline]
    pub fn as_mut_bytes(&mut self) -> &mut [u8; BLOCK_SIZE] {
        &mut self.bytes
    }

    /// Checks if the entire 512-byte sector consists solely of zeroes (End-of-Archive indicator).
    #[inline]
    pub fn is_zero_block(&self) -> bool {
        self.bytes.iter().all(|&b| b == 0)
    }

    /// Returns a reference to the header as an `OldHeader` structure.
    #[inline]
    pub fn as_old_header(&self) -> &OldHeader {
        unsafe { &*(self.bytes.as_ptr() as *const OldHeader) }
    }

    /// Returns a mutable reference to the header as an `OldHeader` structure.
    #[inline]
    pub fn as_old_header_mut(&mut self) -> &mut OldHeader {
        unsafe { &mut *(self.bytes.as_mut_ptr() as *mut OldHeader) }
    }

    /// Returns a reference to the header as a `UstarHeader` structure.
    #[inline]
    pub fn as_ustar_header(&self) -> &UstarHeader {
        unsafe { &*(self.bytes.as_ptr() as *const UstarHeader) }
    }

    /// Returns a mutable reference to the header as a `UstarHeader` structure.
    #[inline]
    pub fn as_ustar_header_mut(&mut self) -> &mut UstarHeader {
        unsafe { &mut *(self.bytes.as_mut_ptr() as *mut UstarHeader) }
    }

    /// Returns a reference to the header as a `GnuHeader` structure.
    #[inline]
    pub fn as_gnu_header(&self) -> &GnuHeader {
        unsafe { &*(self.bytes.as_ptr() as *const GnuHeader) }
    }

    /// Returns a mutable reference to the header as a `GnuHeader` structure.
    #[inline]
    pub fn as_gnu_header_mut(&mut self) -> &mut GnuHeader {
        unsafe { &mut *(self.bytes.as_mut_ptr() as *mut GnuHeader) }
    }

    /// Returns a reference to the header as a `GnuExtSparseHeader` structure.
    #[inline]
    pub fn as_gnu_ext_sparse_header(&self) -> &GnuExtSparseHeader {
        unsafe { &*(self.bytes.as_ptr() as *const GnuExtSparseHeader) }
    }

    /// Returns a mutable reference to the header as a `GnuExtSparseHeader` structure.
    #[inline]
    pub fn as_gnu_ext_sparse_header_mut(&mut self) -> &mut GnuExtSparseHeader {
        unsafe { &mut *(self.bytes.as_mut_ptr() as *mut GnuExtSparseHeader) }
    }

    // --- Raw Byte Field Slice Getters ---

    #[inline]
    pub fn name_bytes(&self) -> &[u8] {
        &self.bytes[OFFSET_NAME..OFFSET_NAME + LEN_NAME]
    }

    #[inline]
    pub fn mode_bytes(&self) -> &[u8] {
        &self.bytes[OFFSET_MODE..OFFSET_MODE + LEN_MODE]
    }

    #[inline]
    pub fn uid_bytes(&self) -> &[u8] {
        &self.bytes[OFFSET_UID..OFFSET_UID + LEN_UID]
    }

    #[inline]
    pub fn gid_bytes(&self) -> &[u8] {
        &self.bytes[OFFSET_GID..OFFSET_GID + LEN_GID]
    }

    #[inline]
    pub fn size_bytes(&self) -> &[u8] {
        &self.bytes[OFFSET_SIZE..OFFSET_SIZE + LEN_SIZE]
    }

    #[inline]
    pub fn mtime_bytes(&self) -> &[u8] {
        &self.bytes[OFFSET_MTIME..OFFSET_MTIME + LEN_MTIME]
    }

    #[inline]
    pub fn chksum_bytes(&self) -> &[u8] {
        &self.bytes[OFFSET_CHKSUM..OFFSET_CHKSUM + LEN_CHKSUM]
    }

    #[inline]
    pub fn typeflag_byte(&self) -> u8 {
        self.bytes[OFFSET_TYPEFLAG]
    }

    #[inline]
    pub fn linkname_bytes(&self) -> &[u8] {
        &self.bytes[OFFSET_LINKNAME..OFFSET_LINKNAME + LEN_LINKNAME]
    }

    #[inline]
    pub fn magic_bytes(&self) -> &[u8] {
        &self.bytes[OFFSET_MAGIC..OFFSET_MAGIC + LEN_MAGIC]
    }

    #[inline]
    pub fn version_bytes(&self) -> &[u8] {
        &self.bytes[OFFSET_VERSION..OFFSET_VERSION + LEN_VERSION]
    }

    #[inline]
    pub fn uname_bytes(&self) -> &[u8] {
        &self.bytes[OFFSET_UNAME..OFFSET_UNAME + LEN_UNAME]
    }

    #[inline]
    pub fn gname_bytes(&self) -> &[u8] {
        &self.bytes[OFFSET_GNAME..OFFSET_GNAME + LEN_GNAME]
    }

    #[inline]
    pub fn devmajor_bytes(&self) -> &[u8] {
        &self.bytes[OFFSET_DEVMAJOR..OFFSET_DEVMAJOR + LEN_DEVMAJOR]
    }

    #[inline]
    pub fn devminor_bytes(&self) -> &[u8] {
        &self.bytes[OFFSET_DEVMINOR..OFFSET_DEVMINOR + LEN_DEVMINOR]
    }

    #[inline]
    pub fn prefix_bytes(&self) -> &[u8] {
        &self.bytes[OFFSET_PREFIX..OFFSET_PREFIX + LEN_PREFIX]
    }

    // --- High-Level Parsed Accessors ---

    /// Extracts null-terminated UTF-8 name from the header.
    #[inline]
    pub fn name(&self) -> &str {
        null_trimmed_str(self.name_bytes())
    }

    /// Sets the name field, truncating at 100 bytes.
    pub fn set_name(&mut self, name: &str) {
        let dest = &mut self.bytes[OFFSET_NAME..OFFSET_NAME + LEN_NAME];
        dest.fill(0);
        let src = name.as_bytes();
        let copy_len = src.len().min(LEN_NAME);
        dest[..copy_len].copy_from_slice(&src[..copy_len]);
    }

    /// Extracts file permissions mode.
    #[inline]
    pub fn mode(&self) -> u32 {
        octal_from(self.mode_bytes()).unwrap_or(0o644) as u32
    }

    /// Sets file permissions mode formatted as octal.
    #[inline]
    pub fn set_mode(&mut self, mode: u32) {
        octal_into(&mut self.bytes[OFFSET_MODE..OFFSET_MODE + LEN_MODE], mode as u64);
    }

    /// Extracts owner UID (with GNU base-256 support).
    #[inline]
    pub fn uid(&self) -> u64 {
        numeric_extended_from(self.uid_bytes())
    }

    /// Sets owner UID (with GNU base-256 support for values >= 8GB/2^21).
    #[inline]
    pub fn set_uid(&mut self, uid: u64) {
        numeric_extended_into(&mut self.bytes[OFFSET_UID..OFFSET_UID + LEN_UID], uid);
    }

    /// Extracts owner GID (with GNU base-256 support).
    #[inline]
    pub fn gid(&self) -> u64 {
        numeric_extended_from(self.gid_bytes())
    }

    /// Sets owner GID (with GNU base-256 support).
    #[inline]
    pub fn set_gid(&mut self, gid: u64) {
        numeric_extended_into(&mut self.bytes[OFFSET_GID..OFFSET_GID + LEN_GID], gid);
    }

    /// Extracts payload file size in bytes (supporting >8GB via GNU base-256).
    #[inline]
    pub fn size(&self) -> u64 {
        numeric_extended_from(self.size_bytes())
    }

    /// Sets payload file size in bytes (supporting >8GB via GNU base-256).
    #[inline]
    pub fn set_size(&mut self, size: u64) {
        numeric_extended_into(&mut self.bytes[OFFSET_SIZE..OFFSET_SIZE + LEN_SIZE], size);
    }

    /// Extracts modification time in seconds since Unix epoch.
    #[inline]
    pub fn mtime(&self) -> u64 {
        numeric_extended_from(self.mtime_bytes())
    }

    /// Sets modification time in seconds since Unix epoch.
    #[inline]
    pub fn set_mtime(&mut self, mtime: u64) {
        numeric_extended_into(&mut self.bytes[OFFSET_MTIME..OFFSET_MTIME + LEN_MTIME], mtime);
    }

    /// Extracts checksum value stored in the header.
    #[inline]
    pub fn chksum(&self) -> u32 {
        octal_from(self.chksum_bytes()).unwrap_or(0) as u32
    }

    /// Returns the strongly-typed `TarEntryType`.
    #[inline]
    pub fn entry_type(&self) -> TarEntryType {
        TarEntryType::from_byte(self.typeflag_byte())
    }

    /// Sets the entry typeflag byte.
    #[inline]
    pub fn set_entry_type(&mut self, entry_type: TarEntryType) {
        self.bytes[OFFSET_TYPEFLAG] = entry_type.as_byte();
    }

    /// Extracts null-terminated link target path.
    #[inline]
    pub fn linkname(&self) -> &str {
        null_trimmed_str(self.linkname_bytes())
    }

    /// Sets link target path.
    pub fn set_linkname(&mut self, linkname: &str) {
        let dest = &mut self.bytes[OFFSET_LINKNAME..OFFSET_LINKNAME + LEN_LINKNAME];
        dest.fill(0);
        let src = linkname.as_bytes();
        let copy_len = src.len().min(LEN_LINKNAME);
        dest[..copy_len].copy_from_slice(&src[..copy_len]);
    }

    /// Extracts null-terminated user name string.
    #[inline]
    pub fn uname(&self) -> &str {
        null_trimmed_str(self.uname_bytes())
    }

    /// Sets user name string.
    pub fn set_uname(&mut self, uname: &str) {
        let dest = &mut self.bytes[OFFSET_UNAME..OFFSET_UNAME + LEN_UNAME];
        dest.fill(0);
        let src = uname.as_bytes();
        let copy_len = src.len().min(LEN_UNAME);
        dest[..copy_len].copy_from_slice(&src[..copy_len]);
    }

    /// Extracts null-terminated group name string.
    #[inline]
    pub fn gname(&self) -> &str {
        null_trimmed_str(self.gname_bytes())
    }

    /// Sets group name string.
    pub fn set_gname(&mut self, gname: &str) {
        let dest = &mut self.bytes[OFFSET_GNAME..OFFSET_GNAME + LEN_GNAME];
        dest.fill(0);
        let src = gname.as_bytes();
        let copy_len = src.len().min(LEN_GNAME);
        dest[..copy_len].copy_from_slice(&src[..copy_len]);
    }

    /// Extracts device major number.
    #[inline]
    pub fn devmajor(&self) -> u32 {
        octal_from(self.devmajor_bytes()).unwrap_or(0) as u32
    }

    /// Sets device major number.
    #[inline]
    pub fn set_devmajor(&mut self, devmajor: u32) {
        if devmajor > 0 {
            octal_into(&mut self.bytes[OFFSET_DEVMAJOR..OFFSET_DEVMAJOR + LEN_DEVMAJOR], devmajor as u64);
        } else {
            self.bytes[OFFSET_DEVMAJOR..OFFSET_DEVMAJOR + LEN_DEVMAJOR].fill(0);
        }
    }

    /// Extracts device minor number.
    #[inline]
    pub fn devminor(&self) -> u32 {
        octal_from(self.devminor_bytes()).unwrap_or(0) as u32
    }

    /// Sets device minor number.
    #[inline]
    pub fn set_devminor(&mut self, devminor: u32) {
        if devminor > 0 {
            octal_into(&mut self.bytes[OFFSET_DEVMINOR..OFFSET_DEVMINOR + LEN_DEVMINOR], devminor as u64);
        } else {
            self.bytes[OFFSET_DEVMINOR..OFFSET_DEVMINOR + LEN_DEVMINOR].fill(0);
        }
    }

    /// Extracts null-terminated USTAR path prefix.
    #[inline]
    pub fn prefix(&self) -> &str {
        null_trimmed_str(self.prefix_bytes())
    }

    /// Sets USTAR path prefix.
    pub fn set_prefix(&mut self, prefix: &str) {
        let dest = &mut self.bytes[OFFSET_PREFIX..OFFSET_PREFIX + LEN_PREFIX];
        dest.fill(0);
        let src = prefix.as_bytes();
        let copy_len = src.len().min(LEN_PREFIX);
        dest[..copy_len].copy_from_slice(&src[..copy_len]);
    }

    /// Returns `true` if magic matches standard POSIX USTAR (`"ustar\0"` or `"ustar"`).
    #[inline]
    pub fn is_ustar(&self) -> bool {
        self.magic_bytes().starts_with(b"ustar") && self.bytes[OFFSET_MAGIC + 5] == 0
    }

    /// Returns `true` if magic matches standard GNU TAR (`"ustar "` with space).
    #[inline]
    pub fn is_gnu(&self) -> bool {
        self.magic_bytes() == MAGIC_GNU
    }

    /// Sets the standard POSIX USTAR magic (`b"ustar\0"`) and version (`b"00"`).
    #[inline]
    pub fn set_ustar_magic(&mut self) {
        self.bytes[OFFSET_MAGIC..OFFSET_MAGIC + LEN_MAGIC].copy_from_slice(MAGIC_USTAR);
        self.bytes[OFFSET_VERSION..OFFSET_VERSION + LEN_VERSION].copy_from_slice(VERSION_USTAR);
    }

    /// Sets the standard GNU TAR magic (`b"ustar "`) and version (`b" \0"`).
    #[inline]
    pub fn set_gnu_magic(&mut self) {
        self.bytes[OFFSET_MAGIC..OFFSET_MAGIC + LEN_MAGIC].copy_from_slice(MAGIC_GNU);
        self.bytes[OFFSET_VERSION..OFFSET_VERSION + LEN_VERSION].copy_from_slice(VERSION_GNU);
    }

    // --- Checksum Computations ---

    /// Dual-mode unsigned and signed octal checksum computation over the 512-byte sector.
    ///
    /// Checksum bytes (148..156) are treated as 8 ASCII space characters (0x20) per TAR specification.
    #[inline]
    pub fn compute_checksum(&self) -> (u32, i32) {
        let mut unsigned_sum: u32 = 0;
        let mut signed_sum: i32 = 0;

        for (i, &b) in self.bytes.iter().enumerate() {
            let val = if (OFFSET_CHKSUM..OFFSET_CHKSUM + LEN_CHKSUM).contains(&i) {
                0x20u8
            } else {
                b
            };
            unsigned_sum += val as u32;
            signed_sum += (val as i8) as i32;
        }

        (unsigned_sum, signed_sum)
    }

    /// Verifies if the checksum stored in the header matches either the unsigned or signed computation.
    #[inline]
    pub fn verify_checksum(&self) -> bool {
        let stored = match octal_from(self.chksum_bytes()) {
            Some(v) => v as u32,
            None => return false,
        };
        let (unsigned_sum, signed_sum) = self.compute_checksum();
        stored == unsigned_sum || (stored as i32) == signed_sum
    }

    /// Recalculates and updates the checksum field formatted as 6-digit octal + null + space.
    pub fn update_checksum(&mut self) {
        let (unsigned_sum, _) = self.compute_checksum();
        let chk_bytes = &mut self.bytes[OFFSET_CHKSUM..OFFSET_CHKSUM + LEN_CHKSUM];
        // Standard TAR: 6 octal digits + '\0' + ' '
        let formatted = format!("{:06o}\0 ", unsigned_sum);
        chk_bytes.copy_from_slice(formatted.as_bytes());
    }
}
