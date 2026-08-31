// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! TAR fixed-length structural geometry, field layout constants, and strong entry types.

/// Standard TAR sector/block size in bytes (512 bytes).
pub const BLOCK_SIZE: usize = 512;

/// Standard USTAR magic bytes (`b"ustar\0"`).
pub const MAGIC_USTAR: &[u8; 6] = b"ustar\0";

/// Standard GNU TAR magic bytes (`b"ustar "` with trailing space).
pub const MAGIC_GNU: &[u8; 6] = b"ustar ";

/// Standard USTAR version bytes (`b"00"`).
pub const VERSION_USTAR: &[u8; 2] = b"00";

/// Standard GNU TAR version bytes (`b" \0"` or `b"00"`).
pub const VERSION_GNU: &[u8; 2] = b" \0";

/// Field offsets in a standard 512-byte TAR header block.
pub const OFFSET_NAME: usize = 0;
pub const OFFSET_MODE: usize = 100;
pub const OFFSET_UID: usize = 108;
pub const OFFSET_GID: usize = 116;
pub const OFFSET_SIZE: usize = 124;
pub const OFFSET_MTIME: usize = 136;
pub const OFFSET_CHKSUM: usize = 148;
pub const OFFSET_TYPEFLAG: usize = 156;
pub const OFFSET_LINKNAME: usize = 157;
pub const OFFSET_MAGIC: usize = 257;
pub const OFFSET_VERSION: usize = 263;
pub const OFFSET_UNAME: usize = 265;
pub const OFFSET_GNAME: usize = 297;
pub const OFFSET_DEVMAJOR: usize = 329;
pub const OFFSET_DEVMINOR: usize = 337;
pub const OFFSET_PREFIX: usize = 345;

/// Field lengths in a standard 512-byte TAR header block.
pub const LEN_NAME: usize = 100;
pub const LEN_MODE: usize = 8;
pub const LEN_UID: usize = 8;
pub const LEN_GID: usize = 8;
pub const LEN_SIZE: usize = 12;
pub const LEN_MTIME: usize = 12;
pub const LEN_CHKSUM: usize = 8;
pub const LEN_TYPEFLAG: usize = 1;
pub const LEN_LINKNAME: usize = 100;
pub const LEN_MAGIC: usize = 6;
pub const LEN_VERSION: usize = 2;
pub const LEN_UNAME: usize = 32;
pub const LEN_GNAME: usize = 32;
pub const LEN_DEVMAJOR: usize = 8;
pub const LEN_DEVMINOR: usize = 8;
pub const LEN_PREFIX: usize = 155;

/// Strong enumeration representing TAR entry types across POSIX, GNU, and PAX standards.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum TarEntryType {
    /// Regular file (`'0'` or `\0`).
    Regular = b'0',
    /// Hard link (`'1'`).
    Link = b'1',
    /// Symbolic link (`'2'`).
    Symlink = b'2',
    /// Character special device (`'3'`).
    Char = b'3',
    /// Block special device (`'4'`).
    Block = b'4',
    /// Directory (`'5'`).
    Directory = b'5',
    /// FIFO pipe (`'6'`).
    Fifo = b'6',
    /// Contiguous file (`'7'`).
    Contiguous = b'7',
    /// GNU long filename metadata entry (`'L'`).
    GNULongName = b'L',
    /// GNU long linkname metadata entry (`'K'`).
    GNULongLink = b'K',
    /// GNU sparse file entry (`'S'`).
    GNUSparse = b'S',
    /// PAX extended header (`'x'`).
    XHeader = b'x',
    /// PAX global extended header (`'g'`).
    XGlobalHeader = b'g',
    /// Solaris extended attribute header (`'X'`).
    SolarisExt = b'X',
    /// Other / unrecognized typeflag byte.
    Other(u8),
}

impl TarEntryType {
    /// Converts a raw ASCII/byte flag into a `TarEntryType`.
    #[inline]
    pub const fn from_byte(b: u8) -> Self {
        match b {
            0 | b'0' => Self::Regular,
            b'1' => Self::Link,
            b'2' => Self::Symlink,
            b'3' => Self::Char,
            b'4' => Self::Block,
            b'5' => Self::Directory,
            b'6' => Self::Fifo,
            b'7' => Self::Contiguous,
            b'L' => Self::GNULongName,
            b'K' => Self::GNULongLink,
            b'S' => Self::GNUSparse,
            b'x' => Self::XHeader,
            b'g' => Self::XGlobalHeader,
            b'X' => Self::SolarisExt,
            other => Self::Other(other),
        }
    }

    /// Converts the `TarEntryType` into its canonical ASCII byte.
    #[inline]
    pub const fn as_byte(self) -> u8 {
        match self {
            Self::Regular => b'0',
            Self::Link => b'1',
            Self::Symlink => b'2',
            Self::Char => b'3',
            Self::Block => b'4',
            Self::Directory => b'5',
            Self::Fifo => b'6',
            Self::Contiguous => b'7',
            Self::GNULongName => b'L',
            Self::GNULongLink => b'K',
            Self::GNUSparse => b'S',
            Self::XHeader => b'x',
            Self::XGlobalHeader => b'g',
            Self::SolarisExt => b'X',
            Self::Other(b) => b,
        }
    }

    /// Returns `true` if this entry represents a regular file.
    #[inline]
    pub const fn is_regular(&self) -> bool {
        matches!(self, Self::Regular | Self::Contiguous)
    }

    /// Returns `true` if this entry represents a directory.
    #[inline]
    pub const fn is_directory(&self) -> bool {
        matches!(self, Self::Directory)
    }

    /// Returns `true` if this entry represents a symbolic link.
    #[inline]
    pub const fn is_symlink(&self) -> bool {
        matches!(self, Self::Symlink)
    }

    /// Returns `true` if this entry represents a hard link.
    #[inline]
    pub const fn is_hardlink(&self) -> bool {
        matches!(self, Self::Link)
    }

    /// Returns `true` if this entry is a PAX extended header (`'x'` or `'g'`).
    #[inline]
    pub const fn is_pax_header(&self) -> bool {
        matches!(self, Self::XHeader | Self::XGlobalHeader)
    }

    /// Returns `true` if this entry is a GNU long name or long link metadata entry.
    #[inline]
    pub const fn is_gnu_long_meta(&self) -> bool {
        matches!(self, Self::GNULongName | Self::GNULongLink)
    }

    /// Returns `true` if this entry represents a GNU sparse file.
    #[inline]
    pub const fn is_sparse(&self) -> bool {
        matches!(self, Self::GNUSparse)
    }
}

impl From<u8> for TarEntryType {
    #[inline]
    fn from(b: u8) -> Self {
        Self::from_byte(b)
    }
}

impl From<TarEntryType> for u8 {
    #[inline]
    fn from(t: TarEntryType) -> Self {
        t.as_byte()
    }
}

/// V7 standard fixed-length header structure (512 bytes).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OldHeader {
    pub name: [u8; 100],
    pub mode: [u8; 8],
    pub uid: [u8; 8],
    pub gid: [u8; 8],
    pub size: [u8; 12],
    pub mtime: [u8; 12],
    pub chksum: [u8; 8],
    pub typeflag: u8,
    pub linkname: [u8; 100],
    pub pad: [u8; 255],
}

/// POSIX.1-1988 Ustar fixed-length header structure (512 bytes).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct UstarHeader {
    pub name: [u8; 100],
    pub mode: [u8; 8],
    pub uid: [u8; 8],
    pub gid: [u8; 8],
    pub size: [u8; 12],
    pub mtime: [u8; 12],
    pub chksum: [u8; 8],
    pub typeflag: u8,
    pub linkname: [u8; 100],
    pub magic: [u8; 6],
    pub version: [u8; 2],
    pub uname: [u8; 32],
    pub gname: [u8; 32],
    pub devmajor: [u8; 8],
    pub devminor: [u8; 8],
    pub prefix: [u8; 155],
    pub pad: [u8; 12],
}

/// GNU Sparse header block component (24 bytes = 12B offset + 12B numbytes).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct GnuSparseHeader {
    pub offset: [u8; 12],
    pub numbytes: [u8; 12],
}

/// GNU TAR header structure with embedded sparse entries and timestamps (512 bytes).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GnuHeader {
    pub name: [u8; 100],
    pub mode: [u8; 8],
    pub uid: [u8; 8],
    pub gid: [u8; 8],
    pub size: [u8; 12],
    pub mtime: [u8; 12],
    pub chksum: [u8; 8],
    pub typeflag: u8,
    pub linkname: [u8; 100],
    pub magic: [u8; 6],
    pub version: [u8; 2],
    pub uname: [u8; 32],
    pub gname: [u8; 32],
    pub devmajor: [u8; 8],
    pub devminor: [u8; 8],
    pub atime: [u8; 12],
    pub ctime: [u8; 12],
    pub offset: [u8; 12],
    pub longnames: [u8; 4],
    pub unused: u8,
    pub sparse: [GnuSparseHeader; 4],
    pub isextended: u8,
    pub realsize: [u8; 12],
    pub pad: [u8; 17],
}

/// GNU Extended Sparse Header block containing 21 sparse entries (512 bytes).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GnuExtSparseHeader {
    pub sparse: [GnuSparseHeader; 21],
    pub isextended: u8,
    pub pad: [u8; 7],
}
