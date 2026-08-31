// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! SCHILY.xattr and LIBARCHIVE.xattr Extended Attributes parsing, formatting, and OS-native bridge.
//!
//! Provides zero-loss parsing and generation of POSIX.1-2001 PAX extended headers containing
//! extended file attributes (xattrs), alongside platform-native restoration with symlink escape defense.

use std::io;
use std::path::Path;

use super::pax::PaxRecord;

/// Standard SCHILY Extended Attribute prefix (`"SCHILY.xattr."`).
pub const SCHILY_XATTR_PREFIX: &str = "SCHILY.xattr.";

/// Standard LIBARCHIVE Extended Attribute prefix (`"LIBARCHIVE.xattr."`).
pub const LIBARCHIVE_XATTR_PREFIX: &str = "LIBARCHIVE.xattr.";

// --- Well-Known macOS Extended Attribute Keys ---

/// macOS Finder Information (32-byte struct containing file type, creator, color flags, stationery).
pub const XATTR_MACOS_FINDER_INFO: &str = "com.apple.FinderInfo";

/// macOS Gatekeeper quarantine metadata string.
pub const XATTR_MACOS_QUARANTINE: &str = "com.apple.quarantine";

/// macOS Spotlight user tags (OpenStep plist / binary serialized tags).
pub const XATTR_MACOS_USER_TAGS: &str = "com.apple.metadata:kMDItemUserTags";

/// macOS Resource Fork payload data.
pub const XATTR_MACOS_RESOURCE_FORK: &str = "com.apple.ResourceFork";

/// macOS Mandatory Access Control Label.
pub const XATTR_MACOS_MACL: &str = "com.apple.macl";

// --- Well-Known Linux Extended Attribute Keys ---

/// Linux SELinux security context.
pub const XATTR_LINUX_SELINUX: &str = "security.selinux";

/// Linux POSIX Access Control List (ACL).
pub const XATTR_LINUX_POSIX_ACL_ACCESS: &str = "system.posix_acl_access";

/// Linux POSIX Default Access Control List (ACL).
pub const XATTR_LINUX_POSIX_ACL_DEFAULT: &str = "system.posix_acl_default";

/// Linux Executable Capability metadata.
pub const XATTR_LINUX_CAPABILITY: &str = "security.capability";

/// Linux User Extended Attribute namespace prefix (`"user."`).
pub const XATTR_LINUX_USER_PREFIX: &str = "user.";

/// Represents a single decoded TAR extended attribute key-value pair.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TarXattr {
    /// The decoded attribute name without `SCHILY.xattr.` or `LIBARCHIVE.xattr.` prefix
    /// (e.g. `"com.apple.FinderInfo"` or `"user.comment"`).
    pub name: String,
    /// The raw byte content of the extended attribute.
    pub value: Vec<u8>,
}

impl TarXattr {
    /// Creates a new `TarXattr` from name and byte value.
    pub fn new(name: impl Into<String>, value: impl Into<Vec<u8>>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }

    /// Creates a new `TarXattr` with string value.
    pub fn from_str_val(name: impl Into<String>, value: &str) -> Self {
        Self {
            name: name.into(),
            value: value.as_bytes().to_vec(),
        }
    }

    /// Returns the attribute name as a string slice.
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the attribute value as a byte slice.
    #[inline]
    pub fn value(&self) -> &[u8] {
        &self.value
    }

    /// Attempts to parse the attribute value as a valid UTF-8 string.
    #[inline]
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.value).ok()
    }

    /// Returns `true` if this attribute belongs to the macOS `com.apple.*` namespace.
    #[inline]
    pub fn is_macos_attribute(&self) -> bool {
        self.name.starts_with("com.apple.")
    }

    /// Returns `true` if this attribute belongs to standard Linux namespaces (`security.*`, `system.*`, `user.*`, `trusted.*`).
    #[inline]
    pub fn is_linux_attribute(&self) -> bool {
        self.name.starts_with("security.")
            || self.name.starts_with("system.")
            || self.name.starts_with("user.")
            || self.name.starts_with("trusted.")
    }

    /// Returns `true` if this is the 32-byte macOS FinderInfo attribute (`com.apple.FinderInfo`).
    #[inline]
    pub fn is_finder_info(&self) -> bool {
        self.name == XATTR_MACOS_FINDER_INFO
    }

    /// Returns `true` if this is macOS Quarantine attribute (`com.apple.quarantine`).
    #[inline]
    pub fn is_quarantine(&self) -> bool {
        self.name == XATTR_MACOS_QUARANTINE
    }

    /// Returns `true` if this is macOS Spotlight User Tags attribute (`com.apple.metadata:kMDItemUserTags`).
    #[inline]
    pub fn is_user_tags(&self) -> bool {
        self.name == XATTR_MACOS_USER_TAGS
    }
}

/// Extracts and normalizes extended attributes from a slice of PAX records.
///
/// Automatically identifies and strips `SCHILY.xattr.` and `LIBARCHIVE.xattr.` prefixes,
/// capturing the raw attribute values with full byte fidelity.
pub fn extract_xattrs_from_pax(records: &[PaxRecord]) -> Vec<TarXattr> {
    let mut xattrs = Vec::new();
    for rec in records {
        if let Some(attr_name) = rec.key.strip_prefix(SCHILY_XATTR_PREFIX) {
            if !attr_name.is_empty() {
                xattrs.push(TarXattr::new(attr_name, rec.value.clone()));
            }
        } else if let Some(attr_name) = rec.key.strip_prefix(LIBARCHIVE_XATTR_PREFIX) {
            if !attr_name.is_empty() {
                xattrs.push(TarXattr::new(attr_name, rec.value.clone()));
            }
        }
    }
    xattrs
}

/// Serializes extended attributes into a concatenated byte stream of standard POSIX.1-2001 PAX records.
///
/// Each record is formatted as `"<total_length> SCHILY.xattr.<name>=<value>\n"` using an exact
/// iterative convergence algorithm to compute variable-length digit prefixes.
pub fn format_xattr_pax_records(xattrs: &[TarXattr]) -> Vec<u8> {
    let mut out = Vec::new();
    for xattr in xattrs {
        let key = format!("{}{}", SCHILY_XATTR_PREFIX, xattr.name);
        // Base length includes space (1), key, '=', value bytes, and newline (1)
        let base_len = 1 + key.len() + 1 + xattr.value.len() + 1;
        let mut total_len = base_len + 2; // Initial 2-digit length estimate

        loop {
            let len_str = total_len.to_string();
            let actual_len = len_str.len() + base_len;
            if actual_len == total_len {
                break;
            }
            total_len = actual_len;
        }

        let header_prefix = format!("{} {}=", total_len, key);
        out.extend_from_slice(header_prefix.as_bytes());
        out.extend_from_slice(&xattr.value);
        out.push(b'\n');
    }
    out
}

/// Parses raw PAX payload byte buffer into structured `PaxRecord` instances.
///
/// Gracefully handles trailing TAR zero-padding and preserves arbitrary binary record values.
pub fn parse_pax_records_from_bytes(data: &[u8]) -> Vec<PaxRecord> {
    let mut records = Vec::new();
    let mut cursor = 0;

    while cursor < data.len() {
        let remaining = &data[cursor..];
        // If remaining bytes are purely NUL padding, terminate parsing
        if remaining.iter().all(|&b| b == 0) {
            break;
        }

        let space_pos = match remaining.iter().position(|&b| b == b' ') {
            Some(p) => p,
            None => break,
        };

        let len_str = match std::str::from_utf8(&remaining[..space_pos]) {
            Ok(s) => s,
            Err(_) => break,
        };

        let record_len: usize = match len_str.parse() {
            Ok(n) if n > space_pos => n,
            _ => break,
        };

        if cursor + record_len > data.len() {
            break;
        }

        let record_bytes = &data[cursor..cursor + record_len];
        cursor += record_len;

        let kv_bytes = if record_bytes.ends_with(b"\n") {
            &record_bytes[space_pos + 1..record_bytes.len() - 1]
        } else {
            &record_bytes[space_pos + 1..]
        };

        let eq_pos = match kv_bytes.iter().position(|&b| b == b'=') {
            Some(p) => p,
            None => continue,
        };

        let key = match std::str::from_utf8(&kv_bytes[..eq_pos]) {
            Ok(k) => k.to_string(),
            Err(_) => continue,
        };

        let value_bytes = kv_bytes[eq_pos + 1..].to_vec();

        records.push(PaxRecord::new(key, value_bytes));
    }

    records
}

/// Extracts extended attributes directly from raw PAX header byte slice.
pub fn extract_xattrs_from_pax_bytes(data: &[u8]) -> Vec<TarXattr> {
    let mut xattrs = Vec::new();
    let mut cursor = 0;

    while cursor < data.len() {
        let remaining = &data[cursor..];
        if remaining.iter().all(|&b| b == 0) {
            break;
        }

        let space_pos = match remaining.iter().position(|&b| b == b' ') {
            Some(p) => p,
            None => break,
        };

        let len_str = match std::str::from_utf8(&remaining[..space_pos]) {
            Ok(s) => s,
            Err(_) => break,
        };

        let record_len: usize = match len_str.parse() {
            Ok(n) if n > space_pos => n,
            _ => break,
        };

        if cursor + record_len > data.len() {
            break;
        }

        let record_bytes = &data[cursor..cursor + record_len];
        cursor += record_len;

        let kv_bytes = if record_bytes.ends_with(b"\n") {
            &record_bytes[space_pos + 1..record_bytes.len() - 1]
        } else {
            &record_bytes[space_pos + 1..]
        };

        let eq_pos = match kv_bytes.iter().position(|&b| b == b'=') {
            Some(p) => p,
            None => continue,
        };

        let key = match std::str::from_utf8(&kv_bytes[..eq_pos]) {
            Ok(k) => k,
            Err(_) => continue,
        };

        let val_bytes = &kv_bytes[eq_pos + 1..];

        if let Some(attr_name) = key.strip_prefix(SCHILY_XATTR_PREFIX) {
            if !attr_name.is_empty() {
                xattrs.push(TarXattr::new(attr_name, val_bytes.to_vec()));
            }
        } else if let Some(attr_name) = key.strip_prefix(LIBARCHIVE_XATTR_PREFIX) {
            if !attr_name.is_empty() {
                xattrs.push(TarXattr::new(attr_name, val_bytes.to_vec()));
            }
        }
    }

    xattrs
}

// --- Platform Native Extended Attributes Restoration and Inspection ---

/// Applies a list of extended attributes to the specified file path using OS-native system calls.
///
/// On macOS: Invokes `libc::setxattr` with `libc::XATTR_NOFOLLOW` flag to strictly prevent symlink escape.
/// On Linux: Invokes `libc::lsetxattr` to safely operate on symlinks without following them.
/// Returns the number of successfully applied attributes.
pub fn apply_xattrs_to_file(path: &Path, xattrs: &[TarXattr]) -> io::Result<usize> {
    if xattrs.is_empty() {
        return Ok(0);
    }

    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let c_path = CString::new(path.as_os_str().as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        let mut applied_count = 0;
        for xattr in xattrs {
            if xattr.name.is_empty() {
                continue;
            }
            let c_name = CString::new(xattr.name.as_bytes())
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

            let ret = unsafe {
                libc::setxattr(
                    c_path.as_ptr(),
                    c_name.as_ptr(),
                    xattr.value.as_ptr() as *const libc::c_void,
                    xattr.value.len(),
                    0,
                    libc::XATTR_NOFOLLOW,
                )
            };

            if ret != 0 {
                return Err(io::Error::last_os_error());
            }
            applied_count += 1;
        }

        Ok(applied_count)
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let c_path = CString::new(path.as_os_str().as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        let mut applied_count = 0;
        for xattr in xattrs {
            if xattr.name.is_empty() {
                continue;
            }
            let c_name = CString::new(xattr.name.as_bytes())
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

            let ret = unsafe {
                libc::lsetxattr(
                    c_path.as_ptr(),
                    c_name.as_ptr(),
                    xattr.value.as_ptr() as *const libc::c_void,
                    xattr.value.len(),
                    0,
                )
            };

            if ret != 0 {
                return Err(io::Error::last_os_error());
            }
            applied_count += 1;
        }

        Ok(applied_count)
    }

    #[cfg(not(unix))]
    {
        let _ = (path, xattrs);
        Ok(0)
    }
}

/// Reads a specific extended attribute by name from a file path using OS-native system calls.
///
/// Returns `Ok(Some(bytes))` if the attribute exists, `Ok(None)` if not found, or `Err` on I/O failure.
pub fn read_xattr_from_file(path: &Path, name: &str) -> io::Result<Option<Vec<u8>>> {
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let c_path = CString::new(path.as_os_str().as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let c_name = CString::new(name.as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        let val_len = unsafe {
            libc::getxattr(
                c_path.as_ptr(),
                c_name.as_ptr(),
                std::ptr::null_mut(),
                0,
                0,
                libc::XATTR_NOFOLLOW,
            )
        };
        if val_len < 0 {
            let err = io::Error::last_os_error();
            // ENOATTR (macOS 93) indicates attribute does not exist
            if err.raw_os_error() == Some(libc::ENOATTR) {
                return Ok(None);
            }
            return Err(err);
        }

        let mut val_buf = vec![0u8; val_len as usize];
        let got_len = unsafe {
            libc::getxattr(
                c_path.as_ptr(),
                c_name.as_ptr(),
                val_buf.as_mut_ptr() as *mut libc::c_void,
                val_buf.len(),
                0,
                libc::XATTR_NOFOLLOW,
            )
        };
        if got_len < 0 {
            return Err(io::Error::last_os_error());
        }
        val_buf.truncate(got_len as usize);
        Ok(Some(val_buf))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let c_path = CString::new(path.as_os_str().as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let c_name = CString::new(name.as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        let val_len = unsafe {
            libc::lgetxattr(
                c_path.as_ptr(),
                c_name.as_ptr(),
                std::ptr::null_mut(),
                0,
            )
        };
        if val_len < 0 {
            let err = io::Error::last_os_error();
            // ENODATA on Linux indicates attribute does not exist
            if err.raw_os_error() == Some(libc::ENODATA) {
                return Ok(None);
            }
            return Err(err);
        }

        let mut val_buf = vec![0u8; val_len as usize];
        let got_len = unsafe {
            libc::lgetxattr(
                c_path.as_ptr(),
                c_name.as_ptr(),
                val_buf.as_mut_ptr() as *mut libc::c_void,
                val_buf.len(),
            )
        };
        if got_len < 0 {
            return Err(io::Error::last_os_error());
        }
        val_buf.truncate(got_len as usize);
        Ok(Some(val_buf))
    }

    #[cfg(not(unix))]
    {
        let _ = (path, name);
        Ok(None)
    }
}

/// Reads all extended attributes from a file path using OS-native system calls.
pub fn read_all_xattrs_from_file(path: &Path) -> io::Result<Vec<TarXattr>> {
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let c_path = CString::new(path.as_os_str().as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        let list_len = unsafe {
            libc::listxattr(c_path.as_ptr(), std::ptr::null_mut(), 0, libc::XATTR_NOFOLLOW)
        };
        if list_len <= 0 {
            return Ok(Vec::new());
        }

        let mut name_buf = vec![0u8; list_len as usize];
        let actual_len = unsafe {
            libc::listxattr(
                c_path.as_ptr(),
                name_buf.as_mut_ptr() as *mut libc::c_char,
                name_buf.len(),
                libc::XATTR_NOFOLLOW,
            )
        };
        if actual_len <= 0 {
            return Ok(Vec::new());
        }

        let mut xattrs = Vec::new();
        let mut cursor = 0;
        while cursor < actual_len as usize {
            let end = match name_buf[cursor..actual_len as usize].iter().position(|&b| b == 0) {
                Some(pos) => cursor + pos,
                None => break,
            };
            if let Ok(name) = std::str::from_utf8(&name_buf[cursor..end]) {
                if let Ok(Some(val)) = read_xattr_from_file(path, name) {
                    xattrs.push(TarXattr::new(name, val));
                }
            }
            cursor = end + 1;
        }

        Ok(xattrs)
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let c_path = CString::new(path.as_os_str().as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        let list_len = unsafe {
            libc::llistxattr(c_path.as_ptr(), std::ptr::null_mut(), 0)
        };
        if list_len <= 0 {
            return Ok(Vec::new());
        }

        let mut name_buf = vec![0u8; list_len as usize];
        let actual_len = unsafe {
            libc::llistxattr(
                c_path.as_ptr(),
                name_buf.as_mut_ptr() as *mut libc::c_char,
                name_buf.len(),
            )
        };
        if actual_len <= 0 {
            return Ok(Vec::new());
        }

        let mut xattrs = Vec::new();
        let mut cursor = 0;
        while cursor < actual_len as usize {
            let end = match name_buf[cursor..actual_len as usize].iter().position(|&b| b == 0) {
                Some(pos) => cursor + pos,
                None => break,
            };
            if let Ok(name) = std::str::from_utf8(&name_buf[cursor..end]) {
                if let Ok(Some(val)) = read_xattr_from_file(path, name) {
                    xattrs.push(TarXattr::new(name, val));
                }
            }
            cursor = end + 1;
        }

        Ok(xattrs)
    }

    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(Vec::new())
    }
}
