// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! ACL Core Types, Identifiers, and Error Definitions.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Error conditions encountered during ACL processing or text parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AclError {
    /// Text format syntax error or invalid token.
    InvalidSyntax(String),
    /// Unrecognized ACL principal tag.
    InvalidTag(String),
    /// Invalid permission character in string representation.
    InvalidPermission(char),
    /// Invalid inheritance or audit flag character.
    InvalidInheritanceFlag(char),
    /// Unrecognized ACE type.
    InvalidAceType(String),
    /// Conversion between ACL models is impossible or lossy without defaults.
    ConversionFailed(String),
}

impl fmt::Display for AclError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSyntax(msg) => write!(f, "Invalid ACL syntax: {}", msg),
            Self::InvalidTag(tag) => write!(f, "Invalid ACL tag: '{}'", tag),
            Self::InvalidPermission(c) => write!(f, "Invalid ACL permission character: '{}'", c),
            Self::InvalidInheritanceFlag(c) => {
                write!(f, "Invalid ACL inheritance flag character: '{}'", c)
            }
            Self::InvalidAceType(t) => write!(f, "Invalid ACL ACE type: '{}'", t),
            Self::ConversionFailed(msg) => write!(f, "ACL conversion failed: {}", msg),
        }
    }
}

impl std::error::Error for AclError {}

/// ACL Model Type specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum AclType {
    /// POSIX.1e draft 17 ACL model (access and default lists).
    #[default]
    Posix1e,
    /// NFSv4 / RFC 7530 / RFC 8881 / ZFS / macOS ACL model.
    Nfs4,
}

/// ACL Principal Tag identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AclTag {
    /// Owning user of the file (`user::` in POSIX, `owner@` in NFSv4).
    UserObj,
    /// Owning group of the file (`group::` in POSIX, `group@` in NFSv4).
    GroupObj,
    /// Named user qualifier (`user:alice:` or `user:1000:`).
    User(String),
    /// Named group qualifier (`group:staff:` or `group:20:`).
    Group(String),
    /// POSIX effective rights mask (`mask::` in POSIX).
    Mask,
    /// World / others (`other::` in POSIX).
    Other,
    /// NFSv4 everyone principal (`everyone@` in NFSv4).
    Everyone,
}

/// NFSv4 Access Control Entry (ACE) type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AceType {
    /// Access allowed.
    Allow,
    /// Access denied.
    Deny,
    /// System audit.
    Audit,
    /// System alarm.
    Alarm,
}
