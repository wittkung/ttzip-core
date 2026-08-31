// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Bitflags controlling security defenses and behavior during archive extraction.

use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not, Sub, SubAssign};

/// Bitflags controlling security defenses and behavior during archive extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SecurityFlags(pub u32);

impl SecurityFlags {
    /// No security flags enabled.
    pub const NONE: Self = Self(0);

    /// Disallow any path segment containing `..` or attempting to traverse above the sandbox root.
    pub const SECURE_NODOTDOT: Self = Self(1 << 0);

    /// Disallow intermediate symlinks and validate symlink targets against sandbox boundaries.
    pub const SECURE_SYMLINKS: Self = Self(1 << 1);

    /// Reject absolute paths (e.g. `/etc/passwd`, `\Windows`, `C:\`).
    pub const SECURE_NOABSOLUTEPATHS: Self = Self(1 << 2);

    /// Atomically unlink existing destination before creating regular files or directories to prevent symlink hijack.
    pub const SECURE_UNLINK_FIRST: Self = Self(1 << 3);

    /// Safely restore POSIX permissions, ownership, and timestamps in a two-stage bottom-up order.
    pub const RESTORE_PERMISSIONS: Self = Self(1 << 4);

    /// All security flags enabled.
    pub const ALL: Self = Self(
        Self::SECURE_NODOTDOT.0
            | Self::SECURE_SYMLINKS.0
            | Self::SECURE_NOABSOLUTEPATHS.0
            | Self::SECURE_UNLINK_FIRST.0
            | Self::RESTORE_PERMISSIONS.0,
    );

    /// Default production security profile.
    pub const DEFAULT: Self = Self::ALL;

    /// Creates an empty set of flags.
    #[inline]
    #[must_use]
    pub const fn empty() -> Self {
        Self::NONE
    }

    /// Creates a set containing all flags.
    #[inline]
    #[must_use]
    pub const fn all() -> Self {
        Self::ALL
    }

    /// Creates flags from raw bits truncated to valid flags.
    #[inline]
    #[must_use]
    pub const fn from_bits_truncate(bits: u32) -> Self {
        Self(bits & Self::ALL.0)
    }

    /// Returns the underlying raw bits.
    #[inline]
    #[must_use]
    pub const fn bits(&self) -> u32 {
        self.0
    }

    /// Returns `true` if all flags in `other` are set.
    #[inline]
    #[must_use]
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns `true` if any flag in `other` is set.
    #[inline]
    #[must_use]
    pub const fn intersects(&self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    /// Returns `true` if no flags are set.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Inserts specified flags.
    #[inline]
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    /// Removes specified flags.
    #[inline]
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    /// Toggles specified flags.
    #[inline]
    pub fn toggle(&mut self, other: Self) {
        self.0 ^= other.0;
    }

    /// Returns the union of two flag sets.
    #[inline]
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns the intersection of two flag sets.
    #[inline]
    #[must_use]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Returns the difference of two flag sets.
    #[inline]
    #[must_use]
    pub const fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

impl BitOr for SecurityFlags {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for SecurityFlags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for SecurityFlags {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for SecurityFlags {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl BitXor for SecurityFlags {
    type Output = Self;
    #[inline]
    fn bitxor(self, rhs: Self) -> Self {
        Self(self.0 ^ rhs.0)
    }
}

impl BitXorAssign for SecurityFlags {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

impl Not for SecurityFlags {
    type Output = Self;
    #[inline]
    fn not(self) -> Self {
        Self(!self.0 & Self::ALL.0)
    }
}

impl Sub for SecurityFlags {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 & !rhs.0)
    }
}

impl SubAssign for SecurityFlags {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.0 &= !rhs.0;
    }
}
