// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! AppleDouble Version 2.0 Header and Entry parser and serializer.

use super::finder_info::FinderInfo;
use super::types::*;

/// AppleDouble Entry Descriptor describing a data slice in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppleDoubleEntryDescriptor {
    pub entry_id: u32,
    pub offset: u32,
    pub length: u32,
}

/// Raw parsed AppleDouble header with entry descriptors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppleDoubleHeader {
    pub magic: u32,
    pub version: u32,
    pub home_fs: [u8; 16],
    pub num_entries: u16,
    pub entries: Vec<AppleDoubleEntryDescriptor>,
}

impl AppleDoubleHeader {
    /// Encodes the header and entry descriptors into a binary byte vector.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let total_header_len = APPLEDOUBLE_HEADER_BASE_SIZE
            + self.entries.len() * APPLEDOUBLE_ENTRY_DESCRIPTOR_SIZE;
        let mut buf = Vec::with_capacity(total_header_len);

        buf.extend_from_slice(&self.magic.to_be_bytes());
        buf.extend_from_slice(&self.version.to_be_bytes());
        buf.extend_from_slice(&self.home_fs);
        buf.extend_from_slice(&(self.entries.len() as u16).to_be_bytes());

        for entry in &self.entries {
            buf.extend_from_slice(&entry.entry_id.to_be_bytes());
            buf.extend_from_slice(&entry.offset.to_be_bytes());
            buf.extend_from_slice(&entry.length.to_be_bytes());
        }

        buf
    }

    /// Decodes an AppleDouble header and descriptors from a binary buffer.
    pub fn decode(data: &[u8]) -> Result<Self, MacMetadataError> {
        if data.len() < APPLEDOUBLE_HEADER_BASE_SIZE {
            return Err(MacMetadataError::BufferTooShort {
                required: APPLEDOUBLE_HEADER_BASE_SIZE,
                actual: data.len(),
            });
        }

        let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        if magic != APPLEDOUBLE_MAGIC && magic != APPLESINGLE_MAGIC {
            return Err(MacMetadataError::InvalidMagic(magic));
        }

        let version = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        if version != APPLEDOUBLE_VERSION_2 {
            return Err(MacMetadataError::UnsupportedVersion(version));
        }

        let mut home_fs = [0u8; 16];
        home_fs.copy_from_slice(&data[8..24]);

        let num_entries = u16::from_be_bytes([data[24], data[25]]);
        let required_header_len = APPLEDOUBLE_HEADER_BASE_SIZE
            + (num_entries as usize) * APPLEDOUBLE_ENTRY_DESCRIPTOR_SIZE;

        if data.len() < required_header_len {
            return Err(MacMetadataError::BufferTooShort {
                required: required_header_len,
                actual: data.len(),
            });
        }

        let mut entries = Vec::with_capacity(num_entries as usize);
        let mut cursor = APPLEDOUBLE_HEADER_BASE_SIZE;

        for _ in 0..num_entries {
            let entry_id = u32::from_be_bytes([
                data[cursor],
                data[cursor + 1],
                data[cursor + 2],
                data[cursor + 3],
            ]);
            let offset = u32::from_be_bytes([
                data[cursor + 4],
                data[cursor + 5],
                data[cursor + 6],
                data[cursor + 7],
            ]);
            let length = u32::from_be_bytes([
                data[cursor + 8],
                data[cursor + 9],
                data[cursor + 10],
                data[cursor + 11],
            ]);

            let end_offset = (offset as usize).saturating_add(length as usize);
            if end_offset > data.len() {
                return Err(MacMetadataError::OffsetOutOfBounds {
                    offset,
                    length,
                    buffer_len: data.len(),
                });
            }

            entries.push(AppleDoubleEntryDescriptor {
                entry_id,
                offset,
                length,
            });
            cursor += APPLEDOUBLE_ENTRY_DESCRIPTOR_SIZE;
        }

        Ok(Self {
            magic,
            version,
            home_fs,
            num_entries,
            entries,
        })
    }
}

/// In-memory representation of an AppleDouble file structure.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AppleDoubleFile {
    pub finder_info: Option<FinderInfo>,
    pub resource_fork: Option<Vec<u8>>,
    pub real_name: Option<String>,
    pub raw_entries: Vec<(u32, Vec<u8>)>,
}

impl AppleDoubleFile {
    /// Creates a new empty AppleDouble structure.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the Finder Info structure.
    #[must_use]
    pub fn with_finder_info(mut self, info: FinderInfo) -> Self {
        self.finder_info = Some(info);
        self
    }

    /// Sets the Resource Fork byte payload.
    #[must_use]
    pub fn with_resource_fork(mut self, rsrc: Vec<u8>) -> Self {
        self.resource_fork = Some(rsrc);
        self
    }

    /// Sets the Real Name string.
    #[must_use]
    pub fn with_real_name(mut self, name: impl Into<String>) -> Self {
        self.real_name = Some(name.into());
        self
    }

    /// Encodes this AppleDouble file into standard Version 2.0 binary representation.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut entry_payloads: Vec<(u32, Vec<u8>)> = Vec::new();

        if let Some(info) = &self.finder_info {
            entry_payloads.push((ENTRY_FINDER_INFO, info.raw().to_vec()));
        }

        if let Some(rsrc) = &self.resource_fork {
            entry_payloads.push((ENTRY_RESOURCE_FORK, rsrc.clone()));
        }

        if let Some(name) = &self.real_name {
            entry_payloads.push((ENTRY_REAL_NAME, name.as_bytes().to_vec()));
        }

        for (id, payload) in &self.raw_entries {
            entry_payloads.push((*id, payload.clone()));
        }

        let num_entries = entry_payloads.len() as u16;
        let header_len = APPLEDOUBLE_HEADER_BASE_SIZE
            + (num_entries as usize) * APPLEDOUBLE_ENTRY_DESCRIPTOR_SIZE;

        let mut descriptors = Vec::with_capacity(entry_payloads.len());
        let mut current_offset = header_len as u32;

        for (entry_id, payload) in &entry_payloads {
            descriptors.push(AppleDoubleEntryDescriptor {
                entry_id: *entry_id,
                offset: current_offset,
                length: payload.len() as u32,
            });
            current_offset += payload.len() as u32;
        }

        let header = AppleDoubleHeader {
            magic: APPLEDOUBLE_MAGIC,
            version: APPLEDOUBLE_VERSION_2,
            home_fs: *DEFAULT_HOME_FS,
            num_entries,
            entries: descriptors,
        };

        let mut output = header.encode();
        for (_, payload) in entry_payloads {
            output.extend_from_slice(&payload);
        }

        output
    }

    /// Decodes an AppleDouble binary byte buffer into an `AppleDoubleFile` struct.
    pub fn decode(data: &[u8]) -> Result<Self, MacMetadataError> {
        let header = AppleDoubleHeader::decode(data)?;
        let mut file = Self::new();

        for entry in header.entries {
            let start = entry.offset as usize;
            let end = start + entry.length as usize;
            if end > data.len() {
                return Err(MacMetadataError::OffsetOutOfBounds {
                    offset: entry.offset,
                    length: entry.length,
                    buffer_len: data.len(),
                });
            }

            let slice = &data[start..end];
            match entry.entry_id {
                ENTRY_FINDER_INFO => {
                    if slice.len() < FINDER_INFO_SIZE {
                        return Err(MacMetadataError::InvalidFinderInfoLength(slice.len()));
                    }
                    let mut raw = [0u8; 32];
                    raw.copy_from_slice(&slice[0..32]);
                    file.finder_info = Some(FinderInfo::from_raw(raw));
                }
                ENTRY_RESOURCE_FORK => {
                    file.resource_fork = Some(slice.to_vec());
                }
                ENTRY_REAL_NAME => {
                    let s = String::from_utf8(slice.to_vec())
                        .map_err(|e| MacMetadataError::Utf8Error(e.to_string()))?;
                    file.real_name = Some(s);
                }
                other_id => {
                    file.raw_entries.push((other_id, slice.to_vec()));
                }
            }
        }

        Ok(file)
    }
}
