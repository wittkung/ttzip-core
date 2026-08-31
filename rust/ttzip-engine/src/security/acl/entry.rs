// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! ACL Entry and ACL aggregation structures with text parsers and formatters.

use super::inheritance::AclInheritance;
use super::permissions::AclPermissions;
use super::types::{AceType, AclError, AclTag, AclType};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// A single entry in an Access Control List.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AclEntry {
    pub tag: AclTag,
    pub permissions: AclPermissions,
    pub inheritance: AclInheritance,
    pub ace_type: AceType,
    pub is_default: bool,
}

impl AclEntry {
    /// Creates a new access ACE entry.
    pub fn new(tag: AclTag, permissions: AclPermissions) -> Self {
        Self {
            tag,
            permissions,
            inheritance: AclInheritance::NONE,
            ace_type: AceType::Allow,
            is_default: false,
        }
    }

    /// Creates a default/inherited ACE entry for directory inheritance in POSIX.1e.
    pub fn new_default(tag: AclTag, permissions: AclPermissions) -> Self {
        Self {
            tag,
            permissions,
            inheritance: AclInheritance::FILE_INHERIT | AclInheritance::DIRECTORY_INHERIT,
            ace_type: AceType::Allow,
            is_default: true,
        }
    }

    /// Formats this entry as a POSIX text line.
    #[must_use]
    pub fn to_posix_text(&self) -> String {
        let prefix = if self.is_default { "default:" } else { "" };
        let tag_str = match &self.tag {
            AclTag::UserObj => "user::".to_string(),
            AclTag::GroupObj => "group::".to_string(),
            AclTag::User(name) => format!("user:{}:", name),
            AclTag::Group(name) => format!("group:{}:", name),
            AclTag::Mask => "mask::".to_string(),
            AclTag::Other => "other::".to_string(),
            AclTag::Everyone => "other::".to_string(),
        };
        format!("{}{}{}", prefix, tag_str, self.permissions.to_posix_string())
    }

    /// Formats this entry as an NFSv4 ACE text line (`tag:qualifier:perms:inheritance:type`).
    #[must_use]
    pub fn to_nfs4_text(&self) -> String {
        let (tag_name, qual) = match &self.tag {
            AclTag::UserObj => ("owner@", ""),
            AclTag::GroupObj => ("group@", ""),
            AclTag::Everyone | AclTag::Other => ("everyone@", ""),
            AclTag::User(name) => ("user", name.as_str()),
            AclTag::Group(name) => ("group", name.as_str()),
            AclTag::Mask => ("mask@", ""),
        };

        let type_str = match self.ace_type {
            AceType::Allow => "allow",
            AceType::Deny => "deny",
            AceType::Audit => "audit",
            AceType::Alarm => "alarm",
        };

        if qual.is_empty() {
            format!(
                "{}:{}:{}:{}",
                tag_name,
                self.permissions.to_nfs4_string(),
                self.inheritance.to_nfs4_string(),
                type_str
            )
        } else {
            format!(
                "{}:{}:{}:{}:{}",
                tag_name,
                qual,
                self.permissions.to_nfs4_string(),
                self.inheritance.to_nfs4_string(),
                type_str
            )
        }
    }
}

/// An Access Control List entity with entries and type classification.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Acl {
    pub acl_type: AclType,
    pub entries: Vec<AclEntry>,
}

impl Acl {
    /// Creates an empty ACL of the specified type.
    pub fn new(acl_type: AclType) -> Self {
        Self {
            acl_type,
            entries: Vec::new(),
        }
    }

    /// Adds an entry to this ACL.
    pub fn add_entry(&mut self, entry: AclEntry) {
        self.entries.push(entry);
    }

    /// Serializes this ACL into standard text format according to its type.
    #[must_use]
    pub fn to_text(&self) -> String {
        match self.acl_type {
            AclType::Posix1e => {
                let mut out = String::new();
                for e in &self.entries {
                    out.push_str(&e.to_posix_text());
                    out.push('\n');
                }
                out
            }
            AclType::Nfs4 => {
                let mut out = String::new();
                for e in &self.entries {
                    out.push_str(&e.to_nfs4_text());
                    out.push('\n');
                }
                out
            }
        }
    }

    /// Parses a POSIX.1e text representation (e.g., `user::rwx\ngroup::r-x\nother::r--`).
    pub fn parse_posix1e(text: &str) -> Result<Self, AclError> {
        let mut acl = Self::new(AclType::Posix1e);
        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let mut is_default = false;
            let mut rem = line;
            if rem.starts_with("default:") {
                is_default = true;
                rem = &rem["default:".len()..];
            } else if rem.starts_with("d:") {
                is_default = true;
                rem = &rem["d:".len()..];
            }

            let parts: Vec<&str> = rem.split(':').collect();
            if parts.len() < 3 {
                return Err(AclError::InvalidSyntax(line.to_string()));
            }

            let tag_type = parts[0];
            let qualifier = parts[1];
            let perm_str = parts[2];

            let tag = match tag_type {
                "u" | "user" => {
                    if qualifier.is_empty() {
                        AclTag::UserObj
                    } else {
                        AclTag::User(qualifier.to_string())
                    }
                }
                "g" | "group" => {
                    if qualifier.is_empty() {
                        AclTag::GroupObj
                    } else {
                        AclTag::Group(qualifier.to_string())
                    }
                }
                "m" | "mask" => AclTag::Mask,
                "o" | "other" => AclTag::Other,
                _ => return Err(AclError::InvalidTag(tag_type.to_string())),
            };

            let permissions = AclPermissions::from_posix_str(perm_str)?;
            let mut entry = AclEntry::new(tag, permissions);
            entry.is_default = is_default;
            if is_default {
                entry.inheritance =
                    AclInheritance::FILE_INHERIT | AclInheritance::DIRECTORY_INHERIT;
            }
            acl.add_entry(entry);
        }
        Ok(acl)
    }

    /// Parses an NFSv4 text representation (e.g., `user:alice:rwxp--a-R-c---:fd----:allow`).
    pub fn parse_nfs4(text: &str) -> Result<Self, AclError> {
        let mut acl = Self::new(AclType::Nfs4);
        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() < 4 {
                return Err(AclError::InvalidSyntax(line.to_string()));
            }

            let (tag, perm_idx, inherit_idx, type_idx) = if parts.len() == 4 {
                let tag = match parts[0] {
                    "owner@" => AclTag::UserObj,
                    "group@" => AclTag::GroupObj,
                    "everyone@" => AclTag::Everyone,
                    "mask@" => AclTag::Mask,
                    _ => return Err(AclError::InvalidTag(parts[0].to_string())),
                };
                (tag, 1, 2, 3)
            } else {
                let tag = match parts[0] {
                    "user" | "u" => AclTag::User(parts[1].to_string()),
                    "group" | "g" => AclTag::Group(parts[1].to_string()),
                    "owner@" => AclTag::UserObj,
                    "group@" => AclTag::GroupObj,
                    "everyone@" => AclTag::Everyone,
                    _ => return Err(AclError::InvalidTag(parts[0].to_string())),
                };
                (tag, 2, 3, 4)
            };

            let permissions = AclPermissions::from_nfs4_str(parts[perm_idx])?;
            let inheritance = AclInheritance::from_nfs4_str(parts[inherit_idx])?;
            let ace_type = match parts[type_idx].to_lowercase().as_str() {
                "allow" => AceType::Allow,
                "deny" => AceType::Deny,
                "audit" => AceType::Audit,
                "alarm" => AceType::Alarm,
                _ => return Err(AclError::InvalidAceType(parts[type_idx].to_string())),
            };

            acl.add_entry(AclEntry {
                tag,
                permissions,
                inheritance,
                ace_type,
                is_default: inheritance.contains(AclInheritance::FILE_INHERIT)
                    || inheritance.contains(AclInheritance::DIRECTORY_INHERIT),
            });
        }
        Ok(acl)
    }
}

impl FromStr for Acl {
    type Err = AclError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains("owner@") || s.contains("everyone@") || s.contains(":allow") || s.contains(":deny") {
            Self::parse_nfs4(s)
        } else {
            Self::parse_posix1e(s)
        }
    }
}
