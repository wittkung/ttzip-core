// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 7-dimensional ACL inheritance and audit flags for NFSv4 ACEs.

use super::types::AclError;
use serde::{Deserialize, Serialize};

/// 7-dimensional ACL inheritance and audit flags for NFSv4 ACEs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct AclInheritance(pub u16);

impl AclInheritance {
    pub const FILE_INHERIT: Self = Self(1 << 0);
    pub const DIRECTORY_INHERIT: Self = Self(1 << 1);
    pub const NO_PROPAGATE: Self = Self(1 << 2);
    pub const INHERIT_ONLY: Self = Self(1 << 3);
    pub const SUCCESSFUL_ACCESS: Self = Self(1 << 4);
    pub const FAILED_ACCESS: Self = Self(1 << 5);
    pub const INHERITED: Self = Self(1 << 6);

    pub const ALL_FLAGS: Self = Self((1 << 7) - 1);
    pub const NONE: Self = Self(0);

    /// Returns true if all flags in `other` are present in `self`.
    #[inline]
    #[must_use]
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns true if no inheritance or audit flags are set.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Converts to standard NFSv4 inheritance string (`fdniSF` or `fdniSFI`).
    #[must_use]
    pub fn to_nfs4_string(&self) -> String {
        const SLOTS: [(u16, char); 6] = [
            (AclInheritance::FILE_INHERIT.0, 'f'),
            (AclInheritance::DIRECTORY_INHERIT.0, 'd'),
            (AclInheritance::NO_PROPAGATE.0, 'n'),
            (AclInheritance::INHERIT_ONLY.0, 'i'),
            (AclInheritance::SUCCESSFUL_ACCESS.0, 'S'),
            (AclInheritance::FAILED_ACCESS.0, 'F'),
        ];

        let mut out = String::with_capacity(7);
        for (bit, ch) in SLOTS {
            if (self.0 & bit) != 0 {
                out.push(ch);
            } else {
                out.push('-');
            }
        }
        if (self.0 & AclInheritance::INHERITED.0) != 0 {
            out.push('I');
        }
        out
    }

    /// Parses NFSv4 inheritance and audit flag characters.
    pub fn from_nfs4_str(s: &str) -> Result<Self, AclError> {
        let mut mask = 0u16;
        for c in s.chars() {
            match c {
                'f' => mask |= Self::FILE_INHERIT.0,
                'd' => mask |= Self::DIRECTORY_INHERIT.0,
                'n' => mask |= Self::NO_PROPAGATE.0,
                'i' => mask |= Self::INHERIT_ONLY.0,
                'S' => mask |= Self::SUCCESSFUL_ACCESS.0,
                'F' => mask |= Self::FAILED_ACCESS.0,
                'I' => mask |= Self::INHERITED.0,
                '-' => {}
                _ => return Err(AclError::InvalidInheritanceFlag(c)),
            }
        }
        Ok(Self(mask))
    }
}

impl std::ops::BitOr for AclInheritance {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for AclInheritance {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}
