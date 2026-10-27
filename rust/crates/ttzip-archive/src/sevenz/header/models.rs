// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Data models for 7z Header metadata.

/// Metadata for an individual file inside a 7z archive.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SevenZFileMeta {
    pub rel_path: String,
    pub is_directory: bool,
    pub is_empty_stream: bool,
    pub mtime_epoch_secs: Option<i64>,
    pub mode: u32,
}

/// Coder description within a 7z Folder block.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SevenZCoder {
    pub method_id: u64,
    pub num_in_streams: u64,
    pub num_out_streams: u64,
    pub properties: Vec<u8>,
}

/// Folder block defining compression and filter chain for a solid stream.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SevenZFolder {
    pub coders: Vec<SevenZCoder>,
    pub bind_pairs: Vec<(u64, u64)>,
    pub packed_streams: Vec<u64>,
    pub unpack_sizes: Vec<u64>,
    pub crc: Option<u32>,
    pub num_unpack_streams: usize,
    pub packed_offset: usize,
    pub packed_len: usize,
}

impl SevenZFolder {
    /// Number of packed streams this folder consumes from the archive pack area.
    ///
    /// Every coder input stream that no bind pair consumes is read straight from the pack area,
    /// so a folder owns `total_in_streams - bind_pairs` packed streams. A folder with a
    /// multi-input coder (such as BCJ2) therefore owns more than one packed stream, and its
    /// packed length is the sum of all of them.
    #[must_use]
    pub fn pack_stream_count(&self) -> usize {
        let total_in = self.coders.iter().fold(0usize, |acc, coder| {
            acc.saturating_add(coder.num_in_streams as usize)
        });
        let num_bind_pairs = self.coders.len().saturating_sub(1);
        total_in.saturating_sub(num_bind_pairs)
    }
}

/// Complete parsed 7z Header information.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SevenZHeaderInfo {
    pub payload_offset: usize,
    pub payload_len: usize,
    pub pack_sizes: Vec<u64>,
    pub folders: Vec<SevenZFolder>,
    pub stream_sizes: Vec<u64>,
    pub stream_crcs: Vec<u32>,
    pub files: Vec<SevenZFileMeta>,
    pub primary_method_id: u64,
    pub coder_props: Vec<u8>,
    pub is_encrypted: bool,
    pub aes_salt: [u8; 16],
    pub aes_salt_len: usize,
    pub aes_iv: [u8; 16],
    pub aes_iv_len: usize,
    pub aes_num_cycles_power: u32,
}
