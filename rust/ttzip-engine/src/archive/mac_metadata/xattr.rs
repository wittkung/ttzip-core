// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Extended attributes collection and OS-native extraction and restoration.

use super::appledouble::AppleDoubleFile;
use super::finder_info::FinderInfo;
use super::types::*;
use std::collections::BTreeMap;
use std::path::Path;

/// Collection of Extended Attributes (xattr) on file system entities.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExtendedAttributes {
    pub attributes: BTreeMap<String, Vec<u8>>,
}

impl ExtendedAttributes {
    /// Creates an empty ExtendedAttributes collection.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets attribute value by key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&[u8]> {
        self.attributes.get(key).map(|v| v.as_slice())
    }

    /// Sets attribute key and value.
    pub fn set(&mut self, key: impl Into<String>, value: Vec<u8>) {
        self.attributes.insert(key.into(), value);
    }

    /// Removes an attribute key, returning previous value if present.
    pub fn remove(&mut self, key: &str) -> Option<Vec<u8>> {
        self.attributes.remove(key)
    }

    /// Returns true if no attributes are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.attributes.is_empty()
    }

    /// Extracts Finder Info structure from `com.apple.FinderInfo`.
    #[must_use]
    pub fn finder_info(&self) -> Option<FinderInfo> {
        let bytes = self.get(XATTR_FINDER_INFO)?;
        if bytes.len() >= 32 {
            let mut raw = [0u8; 32];
            raw.copy_from_slice(&bytes[0..32]);
            Some(FinderInfo::from_raw(raw))
        } else {
            None
        }
    }

    /// Sets `com.apple.FinderInfo` attribute from a `FinderInfo` struct.
    pub fn set_finder_info(&mut self, info: &FinderInfo) {
        self.set(XATTR_FINDER_INFO, info.raw().to_vec());
    }

    /// Extracts Resource Fork payload from `com.apple.ResourceFork`.
    #[must_use]
    pub fn resource_fork(&self) -> Option<&[u8]> {
        self.get(XATTR_RESOURCE_FORK)
    }

    /// Sets `com.apple.ResourceFork` attribute.
    pub fn set_resource_fork(&mut self, rsrc: Vec<u8>) {
        self.set(XATTR_RESOURCE_FORK, rsrc);
    }

    /// Extracts quarantine metadata string from `com.apple.quarantine`.
    #[must_use]
    pub fn quarantine(&self) -> Option<&str> {
        let bytes = self.get(XATTR_QUARANTINE)?;
        std::str::from_utf8(bytes).ok()
    }

    /// Sets quarantine metadata string.
    pub fn set_quarantine(&mut self, q: &str) {
        self.set(XATTR_QUARANTINE, q.as_bytes().to_vec());
    }

    /// Serializes Finder Info and Resource Fork into AppleDouble Version 2.0 format.
    #[must_use]
    pub fn to_appledouble(&self) -> Option<Vec<u8>> {
        let info = self.finder_info();
        let rsrc = self.resource_fork().map(|s| s.to_vec());

        if info.is_none() && rsrc.is_none() {
            return None;
        }

        let mut ad = AppleDoubleFile::new();
        if let Some(i) = info {
            ad.finder_info = Some(i);
        }
        if let Some(r) = rsrc {
            ad.resource_fork = Some(r);
        }

        Some(ad.encode())
    }

    /// Parses an AppleDouble byte slice and populates Finder Info and Resource Fork attributes.
    pub fn from_appledouble(data: &[u8]) -> Result<Self, MacMetadataError> {
        let ad = AppleDoubleFile::decode(data)?;
        let mut xattrs = Self::new();

        if let Some(info) = ad.finder_info {
            xattrs.set_finder_info(&info);
        }
        if let Some(rsrc) = ad.resource_fork {
            xattrs.set_resource_fork(rsrc);
        }

        Ok(xattrs)
    }

    /// Extracts extended attributes from a file path using OS-native APIs.
    pub fn extract_from_path(path: &Path) -> std::io::Result<Self> {
        #[cfg(target_os = "macos")]
        {
            use std::ffi::CString;
            use std::os::unix::ffi::OsStrExt;

            let c_path = CString::new(path.as_os_str().as_bytes())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

            let list_len = unsafe {
                libc::listxattr(c_path.as_ptr(), std::ptr::null_mut(), 0, libc::XATTR_NOFOLLOW)
            };
            if list_len <= 0 {
                return Ok(Self::new());
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
                return Ok(Self::new());
            }

            let mut xattrs = Self::new();
            let mut cursor = 0;
            while cursor < actual_len as usize {
                let end = match name_buf[cursor..actual_len as usize].iter().position(|&b| b == 0) {
                    Some(pos) => cursor + pos,
                    None => break,
                };
                if let Ok(name) = std::str::from_utf8(&name_buf[cursor..end]) {
                    let c_name = CString::new(name).map_err(|e| {
                        std::io::Error::new(std::io::ErrorKind::InvalidInput, e)
                    })?;
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
                    if val_len > 0 {
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
                        if got_len == val_len {
                            xattrs.set(name, val_buf);
                        }
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
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

            let list_len = unsafe {
                libc::listxattr(c_path.as_ptr(), std::ptr::null_mut(), 0)
            };
            if list_len <= 0 {
                return Ok(Self::new());
            }

            let mut name_buf = vec![0u8; list_len as usize];
            let actual_len = unsafe {
                libc::listxattr(
                    c_path.as_ptr(),
                    name_buf.as_mut_ptr() as *mut libc::c_char,
                    name_buf.len(),
                )
            };
            if actual_len <= 0 {
                return Ok(Self::new());
            }

            let mut xattrs = Self::new();
            let mut cursor = 0;
            while cursor < actual_len as usize {
                let end = match name_buf[cursor..actual_len as usize].iter().position(|&b| b == 0) {
                    Some(pos) => cursor + pos,
                    None => break,
                };
                if let Ok(name) = std::str::from_utf8(&name_buf[cursor..end]) {
                    let c_name = CString::new(name).map_err(|e| {
                        std::io::Error::new(std::io::ErrorKind::InvalidInput, e)
                    })?;
                    let val_len = unsafe {
                        libc::getxattr(c_path.as_ptr(), c_name.as_ptr(), std::ptr::null_mut(), 0)
                    };
                    if val_len > 0 {
                        let mut val_buf = vec![0u8; val_len as usize];
                        let got_len = unsafe {
                            libc::getxattr(
                                c_path.as_ptr(),
                                c_name.as_ptr(),
                                val_buf.as_mut_ptr() as *mut libc::c_void,
                                val_buf.len(),
                            )
                        };
                        if got_len == val_len {
                            xattrs.set(name, val_buf);
                        }
                    }
                }
                cursor = end + 1;
            }

            Ok(xattrs)
        }

        #[cfg(not(unix))]
        {
            let _ = path;
            Ok(Self::new())
        }
    }

    /// Applies extended attributes to a target file path using OS-native APIs.
    pub fn apply_to_path(&self, path: &Path) -> std::io::Result<()> {
        #[cfg(target_os = "macos")]
        {
            use std::ffi::CString;
            use std::os::unix::ffi::OsStrExt;

            let c_path = CString::new(path.as_os_str().as_bytes())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

            for (key, val) in &self.attributes {
                let c_key = CString::new(key.as_str()).map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, e)
                })?;
                let ret = unsafe {
                    libc::setxattr(
                        c_path.as_ptr(),
                        c_key.as_ptr(),
                        val.as_ptr() as *const libc::c_void,
                        val.len(),
                        0,
                        libc::XATTR_NOFOLLOW,
                    )
                };
                if ret != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }

            Ok(())
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            use std::ffi::CString;
            use std::os::unix::ffi::OsStrExt;

            let c_path = CString::new(path.as_os_str().as_bytes())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

            for (key, val) in &self.attributes {
                let c_key = CString::new(key.as_str()).map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, e)
                })?;
                let ret = unsafe {
                    libc::setxattr(
                        c_path.as_ptr(),
                        c_key.as_ptr(),
                        val.as_ptr() as *const libc::c_void,
                        val.len(),
                        0,
                    )
                };
                if ret != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }

            Ok(())
        }

        #[cfg(not(unix))]
        {
            let _ = path;
            Ok(())
        }
    }
}
