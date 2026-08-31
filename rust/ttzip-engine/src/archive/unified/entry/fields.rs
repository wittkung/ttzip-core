// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Bitmask flags tracking explicitly populated entry metadata fields.

use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not, Sub, SubAssign};

/// Bitflags tracking which metadata attributes are explicitly populated.
///
/// Differentiates between unset fields and explicitly zero-valued fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct EntryFields(pub u32);

impl EntryFields {
    pub const NONE: Self = Self(0);
    pub const PATHNAME: Self = Self(1 << 0);
    pub const SIZE: Self = Self(1 << 1);
    pub const MTIME: Self = Self(1 << 2);
    pub const ATIME: Self = Self(1 << 3);
    pub const CTIME: Self = Self(1 << 4);
    pub const BIRTHTIME: Self = Self(1 << 5);
    pub const PERMISSIONS: Self = Self(1 << 6);
    pub const FILE_TYPE: Self = Self(1 << 7);
    pub const UID: Self = Self(1 << 8);
    pub const GID: Self = Self(1 << 9);
    pub const UNAME: Self = Self(1 << 10);
    pub const GNAME: Self = Self(1 << 11);
    pub const INO: Self = Self(1 << 12);
    pub const DEV: Self = Self(1 << 13);
    pub const RDEV: Self = Self(1 << 14);
    pub const NLINK: Self = Self(1 << 15);
    pub const SYMLINK: Self = Self(1 << 16);
    pub const HARDLINK: Self = Self(1 << 17);
    pub const XATTRS: Self = Self(1 << 18);
    pub const ACLS: Self = Self(1 << 19);
    pub const SPARSE: Self = Self(1 << 20);
    pub const DIGEST: Self = Self(1 << 21);
    pub const FLAGS: Self = Self(1 << 22);

    pub const UID_GID: Self = Self(Self::UID.0 | Self::GID.0);
    pub const UNAME_GNAME: Self = Self(Self::UNAME.0 | Self::GNAME.0);
    pub const INO_DEV: Self = Self(Self::INO.0 | Self::DEV.0);
    pub const TIMESTAMPS: Self =
        Self(Self::MTIME.0 | Self::ATIME.0 | Self::CTIME.0 | Self::BIRTHTIME.0);
    pub const ALL: Self = Self((1 << 23) - 1);

    #[inline]
    pub const fn empty() -> Self {
        Self::NONE
    }

    #[inline]
    pub const fn all() -> Self {
        Self::ALL
    }

    #[inline]
    pub const fn from_bits_truncate(bits: u32) -> Self {
        Self(bits & Self::ALL.0)
    }

    #[inline]
    pub const fn bits(&self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    #[inline]
    pub const fn intersects(&self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    #[inline]
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    #[inline]
    pub fn toggle(&mut self, other: Self) {
        self.0 ^= other.0;
    }

    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[inline]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    #[inline]
    pub const fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

impl BitOr for EntryFields {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for EntryFields {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for EntryFields {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for EntryFields {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl BitXor for EntryFields {
    type Output = Self;
    #[inline]
    fn bitxor(self, rhs: Self) -> Self {
        Self(self.0 ^ rhs.0)
    }
}

impl BitXorAssign for EntryFields {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

impl Not for EntryFields {
    type Output = Self;
    #[inline]
    fn not(self) -> Self {
        Self(!self.0 & Self::ALL.0)
    }
}

impl Sub for EntryFields {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 & !rhs.0)
    }
}

impl SubAssign for EntryFields {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.0 &= !rhs.0;
    }
}
