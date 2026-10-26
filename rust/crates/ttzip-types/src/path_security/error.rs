// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Strongly-typed zero-allocation error diagnostics for cross-platform system path security.

use std::fmt::{self, Display, Formatter};

/// Zero-heap-allocation error diagnostic for path security validation failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathSecurityError {
    /// Path string is empty.
    Empty,
    /// Path length exceeds maximum permissible length (POSIX PATH_MAX = 4096).
    PathTooLong { len: usize, max: usize },
    /// Path starts with a slash `/` (absolute or root-relative paths are forbidden).
    LeadingSlash,
    /// Path ends with a slash `/` (directory-only terminations are forbidden).
    TrailingSlash,
    /// Path contains an empty component (e.g. consecutive slashes `//`).
    EmptyComponent,
    /// Path component length exceeds maximum single filesystem component limit (255 bytes).
    ComponentTooLong { len: usize, max: usize },
    /// Component starts with leading whitespace.
    LeadingSpace,
    /// Component attempts parent directory traversal `..` (CWE-22 Zip-Slip).
    ParentTraversal,
    /// Component ends with a space or dot (Win32 truncation exploit CWE-41).
    Win32TrailingDotOrSpace,
    /// Component contains adjacent space and dot (Win32 stem truncation defense).
    AdjacentSpaceAndDot,
    /// Component matches `.git` repository folder (case-insensitive).
    GitDirectory,
    /// Component matches Git repository configuration (`.gitmodules` / `.gitattributes`).
    GitMetadata,
    /// Component matches NTFS 8.3 short-name alias pattern (e.g. `~1`).
    ShortNameAlias,
    /// Component matches NTFS filesystem root metadata stream (e.g. `$MFT`, `$LOGFILE`).
    NtfsMetadata,
    /// Component contains a forbidden character (Windows reserved or ASCII control).
    ForbiddenChar(u8),
    /// Component contains Trojan Source bidirectional Unicode override or invisible formatting character.
    TrojanSource,
    /// Component stem matches a Windows reserved device name (`CON`, `PRN`, `AUX`, `NUL`, `COM0..9`, `LPT0..9`, `CONFIG$`, etc.).
    WindowsReservedDevice,
    /// Path contains no normal file component (only current directory `.` references).
    NoNormalComponent,
    /// Path is not valid UTF-8.
    NonUtf8,
}

impl Display for PathSecurityError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "path is empty"),
            Self::PathTooLong { len, max } => {
                write!(f, "path length ({len} bytes) exceeds maximum limit ({max} bytes)")
            }
            Self::LeadingSlash => write!(f, "leading slash is forbidden; path must be relative"),
            Self::TrailingSlash => write!(f, "trailing slash is forbidden; path must target a file"),
            Self::EmptyComponent => write!(f, "consecutive slashes or empty components are forbidden"),
            Self::ComponentTooLong { len, max } => {
                write!(f, "path component length ({len} bytes) exceeds maximum limit ({max} bytes)")
            }
            Self::LeadingSpace => write!(f, "leading whitespace in path component is forbidden"),
            Self::ParentTraversal => write!(f, "parent directory traversal ('..') is strictly forbidden"),
            Self::Win32TrailingDotOrSpace => {
                write!(f, "trailing dot or space in path component is forbidden (Win32 canonicalization defense)")
            }
            Self::AdjacentSpaceAndDot => {
                write!(f, "adjacent space and dot in path component is forbidden (Win32 stem truncation defense)")
            }
            Self::GitDirectory => write!(f, "access to '.git' repository metadata is forbidden"),
            Self::GitMetadata => {
                write!(f, "access to Git repository configuration ('.gitmodules' / '.gitattributes') is forbidden")
            }
            Self::ShortNameAlias => write!(f, "NTFS 8.3 short-name alias ('~n') is forbidden"),
            Self::NtfsMetadata => write!(f, "NTFS system root metadata file ('$MFT' etc.) is forbidden"),
            Self::ForbiddenChar(b) => {
                write!(f, "forbidden character '{}' (0x{b:02X}) in path", b.escape_ascii())
            }
            Self::TrojanSource => {
                write!(f, "Trojan Source / Unicode ignorable code point or invisible formatting character detected")
            }
            Self::WindowsReservedDevice => write!(f, "Windows reserved device name stem is forbidden"),
            Self::NoNormalComponent => write!(f, "path must contain at least one normal file payload component"),
            Self::NonUtf8 => write!(f, "path contains invalid non-UTF-8 bytes"),
        }
    }
}

impl std::error::Error for PathSecurityError {}
