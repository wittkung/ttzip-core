// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Zero-copy streaming scanner for POSIX ustar, GNU TAR, and POSIX.1-2001 PAX archives.
//!
//! Robustly handles GNU LongName/LongLink (Types 'L'/'K'), PAX Extended Headers ('x'/'g'),
//! and double-512-byte zero block End-of-Archive boundaries.

use super::header::*;
use super::pax::{parse_pax_data, PaxAttributes};
use crate::types::TTZipStatus;
use std::borrow::Cow;

/// Parsed zero-copy TAR archive entry descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TarEntry<'a> {
    pub path: Cow<'a, str>,
    pub link_target: Option<Cow<'a, str>>,
    pub size: u64,
    pub mode: u32,
    pub uid: u64,
    pub gid: u64,
    pub mtime_epoch_secs: i64,
    pub mtime_nanos: u32,
    pub typeflag: u8,
    pub uname: Option<Cow<'a, str>>,
    pub gname: Option<Cow<'a, str>>,
    pub data_offset: usize,
    pub header_offset: usize,
    pub is_directory: bool,
    pub is_symlink: bool,
    pub is_hardlink: bool,
    pub pax_attributes: Option<PaxAttributes>,
}

/// Zero-copy TAR archive streaming scanner.
pub struct TarSeekScanner<'a> {
    data: &'a [u8],
    cursor: usize,
    pending_gnu_name: Option<String>,
    pending_gnu_link: Option<String>,
    pending_pax_ext: Option<PaxAttributes>,
    global_pax: Option<PaxAttributes>,
    consecutive_zero_blocks: usize,
}

impl<'a> TarSeekScanner<'a> {
    /// Creates a new streaming scanner over an in-memory archive slice.
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            cursor: 0,
            pending_gnu_name: None,
            pending_gnu_link: None,
            pending_pax_ext: None,
            global_pax: None,
            consecutive_zero_blocks: 0,
        }
    }

    /// Scans and returns all file/directory entries in the archive.
    pub fn scan_all(&mut self) -> Result<Vec<TarEntry<'a>>, TTZipStatus> {
        let mut entries = Vec::new();
        while let Some(entry) = self.next_entry()? {
            entries.push(entry);
        }
        Ok(entries)
    }

    /// Advances to and returns the next regular or special archive entry.
    pub fn next_entry(&mut self) -> Result<Option<TarEntry<'a>>, TTZipStatus> {
        while self.cursor + TAR_BLOCK_SIZE <= self.data.len() {
            let block: &[u8; TAR_BLOCK_SIZE] = self.data[self.cursor..self.cursor + TAR_BLOCK_SIZE]
                .try_into()
                .map_err(|_| TTZipStatus::ErrCorruptHeader)?;

            if is_tar_zero_block(block) {
                self.consecutive_zero_blocks += 1;
                self.cursor += TAR_BLOCK_SIZE;
                if self.consecutive_zero_blocks >= 2 {
                    return Ok(None);
                }
                continue;
            }

            self.consecutive_zero_blocks = 0;
            let header = parse_tar_header_block(block)?;
            let effective_size = self.effective_entry_size(&header);
            let payload_size = usize::try_from(effective_size).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
            let payload_blocks = payload_size.div_ceil(TAR_BLOCK_SIZE) * TAR_BLOCK_SIZE;
            let payload_start = self.skip_gnu_sparse_ext_headers(&header, block, self.cursor + TAR_BLOCK_SIZE);

            if payload_start + payload_blocks > self.data.len() {
                return Err(TTZipStatus::ErrCorruptHeader);
            }

            let payload_slice = &self.data[payload_start..payload_start + payload_size];
            let header_offset = self.cursor;
            self.cursor = payload_start + payload_blocks;

            if self.consume_metadata_header(header.typeflag, payload_slice) {
                continue;
            }

            return Ok(Some(self.build_resolved_tar_entry(header, header_offset, payload_start)));
        }

        Ok(None)
    }

    fn effective_entry_size(&self, header: &TarHeader) -> u64 {
        match header.typeflag {
            TYPE_GNU_LONGNAME
            | TYPE_GNU_LONGLINK
            | TYPE_PAX_EXT_HEADER
            | TYPE_SOLARIS_EXT
            | TYPE_PAX_GLOBAL_HEADER => header.size,
            _ => self
                .pending_pax_ext
                .as_ref()
                .and_then(|p| p.size)
                .unwrap_or(header.size),
        }
    }

    fn skip_gnu_sparse_ext_headers(
        &self,
        header: &TarHeader,
        block: &[u8; TAR_BLOCK_SIZE],
        mut payload_start: usize,
    ) -> usize {
        if header.typeflag == TYPE_GNU_SPARSE {
            let mut is_ext = block[482] != 0;
            while is_ext && payload_start + TAR_BLOCK_SIZE <= self.data.len() {
                let ext_block = &self.data[payload_start..payload_start + TAR_BLOCK_SIZE];
                is_ext = ext_block[504] != 0;
                payload_start += TAR_BLOCK_SIZE;
            }
        }
        payload_start
    }

    fn consume_metadata_header(&mut self, typeflag: u8, payload_slice: &[u8]) -> bool {
        match typeflag {
            TYPE_GNU_LONGNAME => {
                self.pending_gnu_name = Some(parse_null_trimmed_str(payload_slice).to_string());
                true
            }
            TYPE_GNU_LONGLINK => {
                self.pending_gnu_link = Some(parse_null_trimmed_str(payload_slice).to_string());
                true
            }
            TYPE_PAX_EXT_HEADER | TYPE_SOLARIS_EXT => {
                self.pending_pax_ext = Some(parse_pax_data(payload_slice));
                true
            }
            TYPE_PAX_GLOBAL_HEADER => {
                self.global_pax = Some(parse_pax_data(payload_slice));
                true
            }
            _ => false,
        }
    }

    fn build_resolved_tar_entry(
        &mut self,
        header: TarHeader,
        header_offset: usize,
        payload_start: usize,
    ) -> TarEntry<'a> {
        let gnu_name = self.pending_gnu_name.take();
        let gnu_link = self.pending_gnu_link.take();
        let pax_ext = self.pending_pax_ext.take();

        let final_path = self.resolve_entry_path(&header, gnu_name, pax_ext.as_ref());
        let final_link = Self::resolve_entry_link(&header, gnu_link, pax_ext.as_ref());
        let final_size = pax_ext.as_ref().and_then(|p| p.size).unwrap_or(header.size);
        let (final_mtime_secs, final_mtime_nanos) = self.resolve_entry_mtime(&header, pax_ext.as_ref());
        let final_uid = pax_ext.as_ref().and_then(|p| p.uid).unwrap_or(header.uid);
        let final_gid = pax_ext.as_ref().and_then(|p| p.gid).unwrap_or(header.gid);
        let final_uname = Self::resolve_owner_name(pax_ext.as_ref().and_then(|p| p.uname.as_deref()), &header.uname);
        let final_gname = Self::resolve_owner_name(pax_ext.as_ref().and_then(|p| p.gname.as_deref()), &header.gname);
        let is_dir = header.typeflag == TYPE_DIRECTORY || final_path.ends_with('/');

        TarEntry {
            path: Cow::Owned(final_path),
            link_target: final_link,
            size: final_size,
            mode: header.mode,
            uid: final_uid,
            gid: final_gid,
            mtime_epoch_secs: final_mtime_secs,
            mtime_nanos: final_mtime_nanos,
            typeflag: header.typeflag,
            uname: final_uname,
            gname: final_gname,
            data_offset: payload_start,
            header_offset,
            is_directory: is_dir,
            is_symlink: header.typeflag == TYPE_SYMLINK,
            is_hardlink: header.typeflag == TYPE_HARDLINK,
            pax_attributes: pax_ext,
        }
    }

    fn resolve_entry_path(
        &self,
        header: &TarHeader,
        gnu_name: Option<String>,
        pax_ext: Option<&PaxAttributes>,
    ) -> String {
        let raw = if let Some(g_name) = gnu_name {
            g_name
        } else if let Some(p_path) = pax_ext.and_then(|p| p.path.as_ref()) {
            p_path.clone()
        } else if let Some(g_path) = self.global_pax.as_ref().and_then(|p| p.path.as_ref()) {
            g_path.clone()
        } else if !header.prefix.is_empty() {
            format!("{}/{}", header.prefix, header.name)
        } else {
            header.name.clone()
        };
        raw.replace('\\', "/")
    }

    fn resolve_entry_link(
        header: &TarHeader,
        gnu_link: Option<String>,
        pax_ext: Option<&PaxAttributes>,
    ) -> Option<Cow<'a, str>> {
        if let Some(g_link) = gnu_link {
            Some(Cow::Owned(g_link))
        } else if let Some(p_link) = pax_ext.and_then(|p| p.linkpath.as_ref()) {
            Some(Cow::Owned(p_link.clone()))
        } else if !header.linkname.is_empty() {
            Some(Cow::Owned(header.linkname.clone()))
        } else {
            None
        }
    }

    fn resolve_entry_mtime(&self, header: &TarHeader, pax_ext: Option<&PaxAttributes>) -> (i64, u32) {
        let secs = pax_ext
            .and_then(|p| p.mtime_secs)
            .or_else(|| self.global_pax.as_ref().and_then(|p| p.mtime_secs))
            .unwrap_or(header.mtime);
        let nanos = pax_ext
            .and_then(|p| p.mtime_nanos)
            .or_else(|| self.global_pax.as_ref().and_then(|p| p.mtime_nanos))
            .unwrap_or(0);
        (secs, nanos)
    }

    fn resolve_owner_name(pax_name: Option<&str>, header_name: &str) -> Option<Cow<'a, str>> {
        if let Some(s) = pax_name {
            Some(Cow::Owned(s.to_string()))
        } else if !header_name.is_empty() {
            Some(Cow::Owned(header_name.to_string()))
        } else {
            None
        }
    }
}
