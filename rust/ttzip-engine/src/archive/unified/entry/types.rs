// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! POSIX and archive entry file type definitions.

/// POSIX file types supported by the TTZip archive engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TTZipFileType {
    #[default]
    RegularFile,
    Directory,
    Symlink,
    Hardlink,
    Fifo,
    CharacterDevice,
    BlockDevice,
    Socket,
    Unknown,
}

impl TTZipFileType {
    #[inline]
    pub const fn is_file(&self) -> bool {
        matches!(self, Self::RegularFile)
    }

    #[inline]
    pub const fn is_dir(&self) -> bool {
        matches!(self, Self::Directory)
    }

    #[inline]
    pub const fn is_symlink(&self) -> bool {
        matches!(self, Self::Symlink)
    }

    #[inline]
    pub const fn is_hardlink(&self) -> bool {
        matches!(self, Self::Hardlink)
    }

    #[inline]
    pub const fn is_special(&self) -> bool {
        matches!(
            self,
            Self::Fifo | Self::CharacterDevice | Self::BlockDevice | Self::Socket
        )
    }

    /// Converts standard POSIX `st_mode` bits to `TTZipFileType`.
    pub const fn from_posix_mode(mode: u32) -> Self {
        match mode & 0o170000 {
            0o100000 => Self::RegularFile,
            0o040000 => Self::Directory,
            0o120000 => Self::Symlink,
            0o010000 => Self::Fifo,
            0o020000 => Self::CharacterDevice,
            0o060000 => Self::BlockDevice,
            0o140000 => Self::Socket,
            _ => Self::Unknown,
        }
    }

    /// Returns corresponding POSIX file format bitmask.
    pub const fn to_posix_mode_bits(&self) -> u32 {
        match self {
            Self::RegularFile => 0o100000,
            Self::Directory => 0o040000,
            Self::Symlink => 0o120000,
            Self::Hardlink => 0o100000,
            Self::Fifo => 0o010000,
            Self::CharacterDevice => 0o020000,
            Self::BlockDevice => 0o060000,
            Self::Socket => 0o140000,
            Self::Unknown => 0,
        }
    }
}
