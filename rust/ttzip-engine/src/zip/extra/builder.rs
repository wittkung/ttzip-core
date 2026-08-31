// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Serializers for Local File Header (LFH) and Central Directory File Header (CDFH) Extra Fields.

use super::types::*;
use super::ZipExtraFields;

impl ZipExtraFields {
    /// Serializes extra fields for a Local File Header (LFH).
    pub fn build_local_extra(&self) -> Vec<u8> {
        let mut out = Vec::new();

        if let Some(z64) = &self.zip64 {
            out.extend_from_slice(&z64.build_local());
        } else if self.uncompressed_size.is_some() || self.compressed_size.is_some() {
            out.extend_from_slice(&Self::build_zip64_extra(
                self.uncompressed_size,
                self.compressed_size,
                None,
            ));
        }

        if let Some(aes) = &self.winzip_aes {
            out.extend_from_slice(&aes.build());
        } else if self.has_winzip_aes {
            out.extend_from_slice(&Self::build_winzip_aes_extra(self.aes_actual_method));
        }

        if let Some(ts) = &self.extended_timestamp {
            out.extend_from_slice(&ts.build_local());
        } else if let Some(mtime) = self.mod_time {
            out.extend_from_slice(&Self::build_extended_timestamp(mtime));
        }

        if let Some(ux) = &self.infozip_unix_new {
            out.extend_from_slice(&ux.build_local());
        }

        if let Some(ntfs) = &self.ntfs {
            out.extend_from_slice(&ntfs.build());
        }

        if let Some(upath) = &self.unicode_path {
            out.extend_from_slice(&upath.build());
        }

        if let Some(ucomm) = &self.unicode_comment {
            out.extend_from_slice(&ucomm.build());
        }

        if let Some(asi) = &self.asi_unix {
            out.extend_from_slice(&asi.build());
        }

        if let Some(align) = &self.alignment {
            out.extend_from_slice(&align.build_local());
        }

        for (tag, data) in &self.unknown_fields {
            out.extend_from_slice(&tag.to_le_bytes());
            out.extend_from_slice(&(data.len() as u16).to_le_bytes());
            out.extend_from_slice(data);
        }

        out
    }

    /// Serializes extra fields for a Central Directory File Header (CDFH).
    pub fn build_central_extra(&self) -> Vec<u8> {
        let mut out = Vec::new();

        if let Some(z64) = &self.zip64 {
            out.extend_from_slice(&z64.build_central());
        } else if self.uncompressed_size.is_some()
            || self.compressed_size.is_some()
            || self.local_header_offset.is_some()
            || self.disk_number.is_some()
        {
            out.extend_from_slice(&Self::build_zip64_extra(
                self.uncompressed_size,
                self.compressed_size,
                self.local_header_offset,
            ));
        }

        if let Some(aes) = &self.winzip_aes {
            out.extend_from_slice(&aes.build());
        } else if self.has_winzip_aes {
            out.extend_from_slice(&Self::build_winzip_aes_extra(self.aes_actual_method));
        }

        if let Some(ts) = &self.extended_timestamp {
            out.extend_from_slice(&ts.build_central());
        } else if let Some(mtime) = self.mod_time {
            out.extend_from_slice(&Self::build_extended_timestamp(mtime));
        }

        if let Some(ux) = &self.infozip_unix_new {
            out.extend_from_slice(&ux.build_central());
        }

        if let Some(ntfs) = &self.ntfs {
            out.extend_from_slice(&ntfs.build());
        }

        if let Some(upath) = &self.unicode_path {
            out.extend_from_slice(&upath.build());
        }

        if let Some(ucomm) = &self.unicode_comment {
            out.extend_from_slice(&ucomm.build());
        }

        if let Some(asi) = &self.asi_unix {
            out.extend_from_slice(&asi.build());
        }

        // Note: self.alignment is deliberately omitted in Central Directory

        for (tag, data) in &self.unknown_fields {
            out.extend_from_slice(&tag.to_le_bytes());
            out.extend_from_slice(&(data.len() as u16).to_le_bytes());
            out.extend_from_slice(data);
        }

        out
    }

    /// Serializes Zip64 extra field for Central Directory or Local File Header.
    pub fn build_zip64_extra(
        uncompressed_size: Option<u64>,
        compressed_size: Option<u64>,
        local_header_offset: Option<u64>,
    ) -> Vec<u8> {
        let mut payload = Vec::new();
        if let Some(u_sz) = uncompressed_size {
            payload.extend_from_slice(&u_sz.to_le_bytes());
        }
        if let Some(c_sz) = compressed_size {
            payload.extend_from_slice(&c_sz.to_le_bytes());
        }
        if let Some(offset) = local_header_offset {
            payload.extend_from_slice(&offset.to_le_bytes());
        }

        if payload.is_empty() {
            return Vec::new();
        }

        let mut out = Vec::with_capacity(4 + payload.len());
        out.extend_from_slice(&TAG_ZIP64.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        out.extend_from_slice(&payload);
        out
    }

    /// Serializes WinZip AES extra field (`0x9901`).
    pub fn build_winzip_aes_extra(actual_method: u16) -> Vec<u8> {
        WinZipAesExtra::new(actual_method, WINZIP_AES_STRENGTH_256).build()
    }

    /// Serializes Extended Timestamp extra field (`0x5455`).
    pub fn build_extended_timestamp(mtime_secs: u32) -> Vec<u8> {
        let ts = ExtendedTimestampExtra {
            flags: EXT_TIME_FLAG_MTIME,
            mod_time: Some(mtime_secs),
            acc_time: None,
            create_time: None,
        };
        ts.build_local()
    }

    /// Serializes Unicode Path extra field (`0x7075`).
    pub fn build_unicode_path(standard_filename: &str) -> Vec<u8> {
        let field = UnicodeFieldExtra::from_text(
            TAG_UNICODE_PATH,
            standard_filename,
            standard_filename.as_bytes(),
        );
        field.build()
    }
}
