// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 14-dimensional ACL permissions bitmask and mappings.

use super::types::AclError;
use serde::{Deserialize, Serialize};

/// 14-dimensional ACL permissions bitmask aligned with RFC 7530 & POSIX.1e.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct AclPermissions(pub u32);

impl AclPermissions {
    pub const READ_DATA: Self = Self(1 << 0);
    pub const LIST_DIRECTORY: Self = Self(1 << 0);
    pub const WRITE_DATA: Self = Self(1 << 1);
    pub const ADD_FILE: Self = Self(1 << 1);
    pub const APPEND_DATA: Self = Self(1 << 2);
    pub const ADD_SUBDIRECTORY: Self = Self(1 << 2);
    pub const READ_NAMED_ATTRS: Self = Self(1 << 3);
    pub const WRITE_NAMED_ATTRS: Self = Self(1 << 4);
    pub const EXECUTE: Self = Self(1 << 5);
    pub const DELETE_CHILD: Self = Self(1 << 6);
    pub const READ_ATTRIBUTES: Self = Self(1 << 7);
    pub const WRITE_ATTRIBUTES: Self = Self(1 << 8);
    pub const DELETE: Self = Self(1 << 9);
    pub const READ_ACL: Self = Self(1 << 10);
    pub const WRITE_ACL: Self = Self(1 << 11);
    pub const WRITE_OWNER: Self = Self(1 << 12);
    pub const SYNCHRONIZE: Self = Self(1 << 13);

    /// POSIX Read mapping: read data, named attrs, basic attrs, ACL, and synchronize.
    pub const POSIX_READ: Self = Self(
        Self::READ_DATA.0
            | Self::READ_NAMED_ATTRS.0
            | Self::READ_ATTRIBUTES.0
            | Self::READ_ACL.0
            | Self::SYNCHRONIZE.0,
    );

    /// POSIX Write mapping: write data, append data, named attrs, basic attrs, and synchronize.
    pub const POSIX_WRITE: Self = Self(
        Self::WRITE_DATA.0
            | Self::APPEND_DATA.0
            | Self::WRITE_NAMED_ATTRS.0
            | Self::WRITE_ATTRIBUTES.0
            | Self::SYNCHRONIZE.0,
    );

    /// POSIX Execute mapping: execute and synchronize.
    pub const POSIX_EXECUTE: Self = Self(Self::EXECUTE.0 | Self::SYNCHRONIZE.0);

    /// POSIX Read/Write/Execute aggregate.
    pub const POSIX_ALL: Self =
        Self(Self::POSIX_READ.0 | Self::POSIX_WRITE.0 | Self::POSIX_EXECUTE.0);

    /// All 14 NFSv4 permissions mask.
    pub const NFS4_ALL: Self = Self((1 << 14) - 1);

    /// Empty permissions mask.
    pub const NONE: Self = Self(0);

    /// Returns true if all permissions in `other` are set in `self`.
    #[inline]
    #[must_use]
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns true if no permissions are set.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Converts the permissions to a 3-character POSIX string (e.g., "rwx", "r-x", "---").
    #[must_use]
    pub fn to_posix_string(&self) -> String {
        let mut s = String::with_capacity(3);
        s.push(if (self.0 & Self::READ_DATA.0) != 0 {
            'r'
        } else {
            '-'
        });
        s.push(
            if (self.0 & (Self::WRITE_DATA.0 | Self::APPEND_DATA.0)) != 0 {
                'w'
            } else {
                '-'
            },
        );
        s.push(if (self.0 & Self::EXECUTE.0) != 0 {
            'x'
        } else {
            '-'
        });
        s
    }

    /// Parses a 3-character POSIX permission string (e.g. "rwx", "r-x").
    pub fn from_posix_str(s: &str) -> Result<Self, AclError> {
        let mut perms = Self::NONE;
        for c in s.chars() {
            match c {
                'r' | 'R' => perms.0 |= Self::POSIX_READ.0,
                'w' | 'W' => perms.0 |= Self::POSIX_WRITE.0,
                'x' | 'X' => perms.0 |= Self::POSIX_EXECUTE.0,
                '-' => {}
                _ => return Err(AclError::InvalidPermission(c)),
            }
        }
        Ok(perms)
    }

    /// Converts to standard 14-slot NFSv4 permission string (`rwxpDaARWcCos`).
    #[must_use]
    pub fn to_nfs4_string(&self) -> String {
        const SLOTS: [(u32, char); 14] = [
            (AclPermissions::READ_DATA.0, 'r'),
            (AclPermissions::WRITE_DATA.0, 'w'),
            (AclPermissions::EXECUTE.0, 'x'),
            (AclPermissions::APPEND_DATA.0, 'p'),
            (AclPermissions::DELETE_CHILD.0, 'D'),
            (AclPermissions::DELETE.0, 'd'),
            (AclPermissions::READ_ATTRIBUTES.0, 'a'),
            (AclPermissions::WRITE_ATTRIBUTES.0, 'A'),
            (AclPermissions::READ_NAMED_ATTRS.0, 'R'),
            (AclPermissions::WRITE_NAMED_ATTRS.0, 'W'),
            (AclPermissions::READ_ACL.0, 'c'),
            (AclPermissions::WRITE_ACL.0, 'C'),
            (AclPermissions::WRITE_OWNER.0, 'o'),
            (AclPermissions::SYNCHRONIZE.0, 's'),
        ];

        let mut out = String::with_capacity(14);
        for (bit, ch) in SLOTS {
            if (self.0 & bit) != 0 {
                out.push(ch);
            } else {
                out.push('-');
            }
        }
        out
    }

    /// Parses NFSv4 permission characters.
    pub fn from_nfs4_str(s: &str) -> Result<Self, AclError> {
        let mut mask = 0u32;
        for c in s.chars() {
            match c {
                'r' => mask |= Self::READ_DATA.0,
                'w' => mask |= Self::WRITE_DATA.0,
                'p' => mask |= Self::APPEND_DATA.0,
                'R' => mask |= Self::READ_NAMED_ATTRS.0,
                'W' => mask |= Self::WRITE_NAMED_ATTRS.0,
                'x' => mask |= Self::EXECUTE.0,
                'D' => mask |= Self::DELETE_CHILD.0,
                'a' => mask |= Self::READ_ATTRIBUTES.0,
                'A' => mask |= Self::WRITE_ATTRIBUTES.0,
                'd' => mask |= Self::DELETE.0,
                'c' => mask |= Self::READ_ACL.0,
                'C' => mask |= Self::WRITE_ACL.0,
                'o' => mask |= Self::WRITE_OWNER.0,
                's' => mask |= Self::SYNCHRONIZE.0,
                '-' => {}
                _ => return Err(AclError::InvalidPermission(c)),
            }
        }
        Ok(Self(mask))
    }
}

impl std::ops::BitOr for AclPermissions {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for AclPermissions {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl std::ops::Not for AclPermissions {
    type Output = Self;
    #[inline]
    fn not(self) -> Self {
        Self(!self.0 & Self::NFS4_ALL.0)
    }
}
