// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! POSIX.1e and NFSv4 bidirectional state transfer algorithms.

use super::entry::{Acl, AclEntry};
use super::inheritance::AclInheritance;
use super::permissions::AclPermissions;
use super::types::{AceType, AclTag, AclType};

/// Converts a POSIX.1e ACL into an NFSv4 ACL with high-fidelity state transfer.
#[must_use]
pub fn posix1e_to_nfs4(posix_acl: &Acl) -> Acl {
    let mut nfs_acl = Acl::new(AclType::Nfs4);

    for entry in &posix_acl.entries {
        let mut nfs_perms = AclPermissions::NONE;
        if (entry.permissions.0 & AclPermissions::READ_DATA.0) != 0 {
            nfs_perms = nfs_perms | AclPermissions::POSIX_READ;
        }
        if (entry.permissions.0 & (AclPermissions::WRITE_DATA.0 | AclPermissions::APPEND_DATA.0)) != 0 {
            nfs_perms = nfs_perms | AclPermissions::POSIX_WRITE;
        }
        if (entry.permissions.0 & AclPermissions::EXECUTE.0) != 0 {
            nfs_perms = nfs_perms | AclPermissions::POSIX_EXECUTE;
        }

        let nfs_tag = match &entry.tag {
            AclTag::UserObj => AclTag::UserObj,
            AclTag::GroupObj => AclTag::GroupObj,
            AclTag::Other => AclTag::Everyone,
            AclTag::Everyone => AclTag::Everyone,
            AclTag::User(name) => AclTag::User(name.clone()),
            AclTag::Group(name) => AclTag::Group(name.clone()),
            AclTag::Mask => AclTag::Mask,
        };

        let inheritance = if entry.is_default {
            AclInheritance::FILE_INHERIT
                | AclInheritance::DIRECTORY_INHERIT
                | AclInheritance::INHERIT_ONLY
        } else {
            AclInheritance::NONE
        };

        nfs_acl.add_entry(AclEntry {
            tag: nfs_tag,
            permissions: nfs_perms,
            inheritance,
            ace_type: AceType::Allow,
            is_default: entry.is_default,
        });
    }

    nfs_acl
}

/// Converts an NFSv4 ACL into a POSIX.1e ACL with effective rights calculation.
#[must_use]
pub fn nfs4_to_posix1e(nfs_acl: &Acl) -> Acl {
    let mut posix_acl = Acl::new(AclType::Posix1e);
    let mut has_named_entries = false;
    let mut explicit_mask: Option<AclPermissions> = None;
    let mut union_rights = AclPermissions::NONE;

    for entry in &nfs_acl.entries {
        if entry.ace_type != AceType::Allow {
            continue;
        }

        let mut posix_perms = AclPermissions::NONE;
        if (entry.permissions.0 & AclPermissions::READ_DATA.0) != 0 {
            posix_perms = posix_perms | AclPermissions::READ_DATA;
        }
        if (entry.permissions.0 & (AclPermissions::WRITE_DATA.0 | AclPermissions::APPEND_DATA.0)) != 0 {
            posix_perms = posix_perms | AclPermissions::WRITE_DATA;
        }
        if (entry.permissions.0 & AclPermissions::EXECUTE.0) != 0 {
            posix_perms = posix_perms | AclPermissions::EXECUTE;
        }

        let posix_tag = match &entry.tag {
            AclTag::UserObj => AclTag::UserObj,
            AclTag::GroupObj => {
                union_rights = union_rights | posix_perms;
                AclTag::GroupObj
            }
            AclTag::Everyone | AclTag::Other => AclTag::Other,
            AclTag::User(name) => {
                has_named_entries = true;
                union_rights = union_rights | posix_perms;
                AclTag::User(name.clone())
            }
            AclTag::Group(name) => {
                has_named_entries = true;
                union_rights = union_rights | posix_perms;
                AclTag::Group(name.clone())
            }
            AclTag::Mask => {
                explicit_mask = Some(posix_perms);
                AclTag::Mask
            }
        };

        posix_acl.add_entry(AclEntry {
            tag: posix_tag,
            permissions: posix_perms,
            inheritance: entry.inheritance,
            ace_type: AceType::Allow,
            is_default: entry.is_default,
        });
    }

    if has_named_entries && explicit_mask.is_none() {
        let mask_entry = AclEntry::new(AclTag::Mask, union_rights);
        posix_acl.add_entry(mask_entry);
    }

    posix_acl
}
