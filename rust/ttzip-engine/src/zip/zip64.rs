// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zip64 Dual-Threshold Promotion State Machine and LFH/CDFH Type-Safe Orchestrator.
//!
//! Implements PKWARE APPNOTE 6.3.3 Zip64 specification invariants:
//! - Dual thresholds: 4GB byte limit (`0xFFFF_FFFF`) and 65535 entry limit (`0xFFFF`).
//! - LFH Zip64 Extra Field invariant: strictly 16-byte fixed payload (8B uncompressed + 8B compressed).
//!   Local header offset is strictly prohibited in LFH.
//! - CDFH Zip64 Extra Field invariant: dynamic variable length (8B / 16B / 24B / 28B)
//!   conditioned strictly on `0xFFFF_FFFF` / `0xFFFF` sentinel values.
//! - Zip64 EOCD Record & Locator serialization and boundary-checked deserialization.
//! - Deterministic Zip64 Decision Matrix for zero-allocation promotion decisions.

use crate::types::TTZipStatus;

/// Byte threshold for 32-bit field overflow: 4GB - 1 (0xFFFFFFFF).
pub const ZIP64_BYTES_THR: u64 = 0xFFFF_FFFF;

/// Entry count threshold for 16-bit field overflow: 65535 (0xFFFF).
pub const ZIP64_ENTRY_THR: u64 = 0xFFFF;

/// Zip64 Extended Information Extra Field Header ID (`0x0001`).
pub const TAG_ZIP64: u16 = 0x0001;

/// Zip64 End of Central Directory Record signature (`0x06064B50` / "PK\x06\x06").
pub const MAGIC_ZIP64_EOCD: u32 = 0x06064B50;

/// Zip64 End of Central Directory Locator signature (`0x07064B50` / "PK\x06\x07").
pub const MAGIC_ZIP64_LOCATOR: u32 = 0x07064B50;

/// Fixed minimum byte size of a Zip64 EOCD record (excluding optional extensible data).
pub const ZIP64_EOCD_MIN_SIZE: usize = 56;

/// Fixed byte size of a Zip64 EOCD locator.
pub const ZIP64_LOCATOR_SIZE: usize = 20;

/// Fixed byte size of an LFH Zip64 extra field payload (8B uncompressed + 8B compressed).
pub const ZIP64_LFH_PAYLOAD_SIZE: usize = 16;

/// Version needed to extract for Zip64 format (4.5).
pub const ZIP64_VERSION_NEEDED: u16 = 45;

/// Sentinel value for 32-bit fields indicating Zip64 promotion.
pub const SENTINEL_U32: u32 = 0xFFFF_FFFF;

/// Sentinel value for 16-bit fields indicating Zip64 promotion.
pub const SENTINEL_U16: u16 = 0xFFFF;

#[inline(always)]
fn read_u16_le(slice: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([slice[offset], slice[offset + 1]])
}

#[inline(always)]
fn read_u32_le(slice: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        slice[offset],
        slice[offset + 1],
        slice[offset + 2],
        slice[offset + 3],
    ])
}

#[inline(always)]
fn read_u64_le(slice: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        slice[offset],
        slice[offset + 1],
        slice[offset + 2],
        slice[offset + 3],
        slice[offset + 4],
        slice[offset + 5],
        slice[offset + 6],
        slice[offset + 7],
    ])
}

/// Represents the Zip64 End of Central Directory (EOCD) Record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zip64EocdRecord {
    /// Size of the remaining record bytes (record size - 12, minimum 44).
    pub size_of_record: u64,
    /// Version made by (e.g. 0x032D for Unix 4.5).
    pub version_made_by: u16,
    /// Version needed to extract (45 for 4.5).
    pub version_needed: u16,
    /// Number of this disk.
    pub disk_number: u32,
    /// Number of the disk with the start of the Central Directory.
    pub disk_with_cd_start: u32,
    /// Total number of entries in the Central Directory on this disk.
    pub entries_on_this_disk: u64,
    /// Total number of entries in the Central Directory.
    pub total_entries: u64,
    /// Size of the Central Directory in bytes.
    pub cd_size: u64,
    /// Offset of the start of the Central Directory relative to the archive start.
    pub cd_offset: u64,
    /// Extensible data sector (optional).
    pub extensible_data: Vec<u8>,
}

impl Zip64EocdRecord {
    /// Creates a standard single-disk Zip64 EOCD record.
    pub fn new(total_entries: u64, cd_size: u64, cd_offset: u64) -> Self {
        Self {
            size_of_record: 44,
            version_made_by: 0x032D, // Unix + Zip 4.5
            version_needed: ZIP64_VERSION_NEEDED,
            disk_number: 0,
            disk_with_cd_start: 0,
            entries_on_this_disk: total_entries,
            total_entries,
            cd_size,
            cd_offset,
            extensible_data: Vec::new(),
        }
    }

    /// Serializes the Zip64 EOCD record into binary bytes.
    pub fn serialize(&self) -> Vec<u8> {
        let rec_size = 44u64 + self.extensible_data.len() as u64;
        let mut out = Vec::with_capacity(ZIP64_EOCD_MIN_SIZE + self.extensible_data.len());
        out.extend_from_slice(&MAGIC_ZIP64_EOCD.to_le_bytes());
        out.extend_from_slice(&rec_size.to_le_bytes());
        out.extend_from_slice(&self.version_made_by.to_le_bytes());
        out.extend_from_slice(&self.version_needed.to_le_bytes());
        out.extend_from_slice(&self.disk_number.to_le_bytes());
        out.extend_from_slice(&self.disk_with_cd_start.to_le_bytes());
        out.extend_from_slice(&self.entries_on_this_disk.to_le_bytes());
        out.extend_from_slice(&self.total_entries.to_le_bytes());
        out.extend_from_slice(&self.cd_size.to_le_bytes());
        out.extend_from_slice(&self.cd_offset.to_le_bytes());
        if !self.extensible_data.is_empty() {
            out.extend_from_slice(&self.extensible_data);
        }
        out
    }

    /// Parses a Zip64 EOCD record from a byte slice with boundary validation.
    pub fn parse(slice: &[u8]) -> Result<Self, TTZipStatus> {
        if slice.len() < ZIP64_EOCD_MIN_SIZE {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let sig = read_u32_le(slice, 0);
        if sig != MAGIC_ZIP64_EOCD {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let size_of_record = read_u64_le(slice, 4);
        if size_of_record < 44 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let total_expected = 12usize
            .checked_add(size_of_record as usize)
            .ok_or(TTZipStatus::ErrCorruptHeader)?;
        if slice.len() < total_expected {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let version_made_by = read_u16_le(slice, 12);
        let version_needed = read_u16_le(slice, 14);
        let disk_number = read_u32_le(slice, 16);
        let disk_with_cd_start = read_u32_le(slice, 20);
        let entries_on_this_disk = read_u64_le(slice, 24);
        let total_entries = read_u64_le(slice, 32);
        let cd_size = read_u64_le(slice, 40);
        let cd_offset = read_u64_le(slice, 48);

        let extensible_len = (size_of_record as usize) - 44;
        let extensible_data = if extensible_len > 0 {
            slice[56..56 + extensible_len].to_vec()
        } else {
            Vec::new()
        };

        Ok(Self {
            size_of_record,
            version_made_by,
            version_needed,
            disk_number,
            disk_with_cd_start,
            entries_on_this_disk,
            total_entries,
            cd_size,
            cd_offset,
            extensible_data,
        })
    }
}

/// Represents the Zip64 End of Central Directory Locator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zip64EocdLocator {
    /// Number of the disk with the start of the Zip64 EOCD.
    pub disk_with_zip64_eocd: u32,
    /// Relative offset of the Zip64 EOCD record.
    pub zip64_eocd_offset: u64,
    /// Total number of disks.
    pub total_disks: u32,
}

impl Zip64EocdLocator {
    /// Creates a standard single-disk Zip64 EOCD locator.
    pub fn new(zip64_eocd_offset: u64) -> Self {
        Self {
            disk_with_zip64_eocd: 0,
            zip64_eocd_offset,
            total_disks: 1,
        }
    }

    /// Serializes the Zip64 EOCD locator into fixed 20-byte array.
    pub fn serialize(&self) -> [u8; ZIP64_LOCATOR_SIZE] {
        let mut out = [0u8; ZIP64_LOCATOR_SIZE];
        out[0..4].copy_from_slice(&MAGIC_ZIP64_LOCATOR.to_le_bytes());
        out[4..8].copy_from_slice(&self.disk_with_zip64_eocd.to_le_bytes());
        out[8..16].copy_from_slice(&self.zip64_eocd_offset.to_le_bytes());
        out[16..20].copy_from_slice(&self.total_disks.to_le_bytes());
        out
    }

    /// Serializes the Zip64 EOCD locator into byte vector.
    pub fn serialize_vec(&self) -> Vec<u8> {
        self.serialize().to_vec()
    }

    /// Parses a Zip64 EOCD locator from a byte slice with boundary validation.
    pub fn parse(slice: &[u8]) -> Result<Self, TTZipStatus> {
        if slice.len() < ZIP64_LOCATOR_SIZE {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let sig = read_u32_le(slice, 0);
        if sig != MAGIC_ZIP64_LOCATOR {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let disk_with_zip64_eocd = read_u32_le(slice, 4);
        let zip64_eocd_offset = read_u64_le(slice, 8);
        let total_disks = read_u32_le(slice, 16);

        Ok(Self {
            disk_with_zip64_eocd,
            zip64_eocd_offset,
            total_disks,
        })
    }
}

/// Strongly-typed Zip64 Extended Information Extra Field orchestrator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Zip64ExtraField {
    /// Local File Header mode: strictly 16 bytes (8B Uncompressed + 8B Compressed).
    /// Local header offset is forbidden in LFH per PKWARE specification.
    Lfh {
        uncompressed_size: u64,
        compressed_size: u64,
    },
    /// Central Directory File Header mode: dynamic variable length (8B / 16B / 24B / 28B).
    /// Fields appear strictly in PKWARE sequence if corresponding CDFH standard fields contain sentinels.
    Cdfh {
        uncompressed_size: Option<u64>,
        compressed_size: Option<u64>,
        local_header_offset: Option<u64>,
        disk_start_number: Option<u32>,
    },
}

impl Zip64ExtraField {
    /// Builds binary extra field for Local File Header (LFH) mode.
    /// Payload is strictly locked to 16 bytes (total 20 bytes with tag and size headers).
    pub fn build_lfh(uncompressed_size: u64, compressed_size: u64) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + ZIP64_LFH_PAYLOAD_SIZE);
        out.extend_from_slice(&TAG_ZIP64.to_le_bytes());
        out.extend_from_slice(&(ZIP64_LFH_PAYLOAD_SIZE as u16).to_le_bytes());
        out.extend_from_slice(&uncompressed_size.to_le_bytes());
        out.extend_from_slice(&compressed_size.to_le_bytes());
        out
    }

    /// Builds binary extra field for Central Directory File Header (CDFH) mode.
    /// Conditionally appends fields according to PKWARE order.
    pub fn build_cdfh(
        uncompressed_size: Option<u64>,
        compressed_size: Option<u64>,
        local_header_offset: Option<u64>,
        disk_start_number: Option<u32>,
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
        if let Some(disk) = disk_start_number {
            payload.extend_from_slice(&disk.to_le_bytes());
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

    /// Serializes this strongly-typed Zip64 extra field into binary bytes.
    pub fn serialize(&self) -> Vec<u8> {
        match self {
            Self::Lfh {
                uncompressed_size,
                compressed_size,
            } => Self::build_lfh(*uncompressed_size, *compressed_size),
            Self::Cdfh {
                uncompressed_size,
                compressed_size,
                local_header_offset,
                disk_start_number,
            } => Self::build_cdfh(
                *uncompressed_size,
                *compressed_size,
                *local_header_offset,
                *disk_start_number,
            ),
        }
    }

    /// Parses an LFH Zip64 extra field payload slice (must be >= 16 bytes).
    pub fn parse_lfh_payload(payload: &[u8]) -> Result<(u64, u64), TTZipStatus> {
        if payload.len() < ZIP64_LFH_PAYLOAD_SIZE {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let uncomp = read_u64_le(payload, 0);
        let comp = read_u64_le(payload, 8);
        Ok((uncomp, comp))
    }

    /// Parses a CDFH Zip64 extra field payload slice conditioned on CDFH sentinel flags.
    pub fn parse_cdfh_payload(
        payload: &[u8],
        uncomp_sentinel: bool,
        comp_sentinel: bool,
        offset_sentinel: bool,
        disk_sentinel: bool,
    ) -> Result<Self, TTZipStatus> {
        let mut cursor = 0;
        let mut uncompressed_size = None;
        let mut compressed_size = None;
        let mut local_header_offset = None;
        let mut disk_start_number = None;

        if uncomp_sentinel {
            if cursor + 8 > payload.len() {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            uncompressed_size = Some(read_u64_le(payload, cursor));
            cursor += 8;
        }

        if comp_sentinel {
            if cursor + 8 > payload.len() {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            compressed_size = Some(read_u64_le(payload, cursor));
            cursor += 8;
        }

        if offset_sentinel {
            if cursor + 8 > payload.len() {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            local_header_offset = Some(read_u64_le(payload, cursor));
            cursor += 8;
        }

        if disk_sentinel {
            if cursor + 4 > payload.len() {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            disk_start_number = Some(read_u32_le(payload, cursor));
        }

        Ok(Self::Cdfh {
            uncompressed_size,
            compressed_size,
            local_header_offset,
            disk_start_number,
        })
    }
}

/// Promotion decision result for Local File Header (LFH).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LfhZip64Decision {
    /// True if Zip64 promotion is activated for LFH.
    pub is_zip64: bool,
    /// 32-bit uncompressed size field for standard LFH header (`0xFFFFFFFF` if Zip64).
    pub uncompressed_size_field: u32,
    /// 32-bit compressed size field for standard LFH header (`0xFFFFFFFF` if Zip64).
    pub compressed_size_field: u32,
    /// Minimum version needed (45 if Zip64, else 20 for standard Deflate/Store).
    pub version_needed: u16,
    /// Strongly-typed extra field object if Zip64 is required.
    pub extra_field: Option<Zip64ExtraField>,
    /// Pre-serialized binary extra field bytes (empty if not Zip64).
    pub extra_bytes: Vec<u8>,
}

/// Promotion decision result for Central Directory File Header (CDFH).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdfhZip64Decision {
    /// True if Zip64 promotion is activated for CDFH.
    pub is_zip64: bool,
    /// 32-bit uncompressed size field for CDFH (`0xFFFFFFFF` if Zip64).
    pub uncompressed_size_field: u32,
    /// 32-bit compressed size field for CDFH (`0xFFFFFFFF` if Zip64).
    pub compressed_size_field: u32,
    /// 32-bit local header offset field for CDFH (`0xFFFFFFFF` if Zip64).
    pub local_header_offset_field: u32,
    /// Minimum version needed (45 if Zip64, else 20).
    pub version_needed: u16,
    /// Strongly-typed extra field object if Zip64 is required.
    pub extra_field: Option<Zip64ExtraField>,
    /// Pre-serialized binary extra field bytes (empty if not Zip64).
    pub extra_bytes: Vec<u8>,
}

/// Promotion decision result for End of Central Directory (EOCD).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EocdZip64Decision {
    /// True if Zip64 EOCD Record & Locator are required.
    pub is_zip64: bool,
    /// 16-bit total entries field for standard EOCD (`0xFFFF` if Zip64).
    pub total_entries_field: u16,
    /// 32-bit Central Directory size field for standard EOCD (`0xFFFFFFFF` if Zip64).
    pub cd_size_field: u32,
    /// 32-bit Central Directory offset field for standard EOCD (`0xFFFFFFFF` if Zip64).
    pub cd_offset_field: u32,
    /// Zip64 EOCD Record structure if promoted.
    pub zip64_eocd: Option<Zip64EocdRecord>,
    /// Zip64 EOCD Locator structure if promoted.
    pub zip64_locator: Option<Zip64EocdLocator>,
}

/// Deterministic Zip64 Decision Matrix for zero-allocation promotion decisions.
#[derive(Debug, Clone, Copy, Default)]
pub struct Zip64DecisionMatrix;

impl Zip64DecisionMatrix {
    /// Evaluates Local File Header (LFH) promotion criteria.
    ///
    /// Per PKWARE APPNOTE: In LFH, if either uncompressed size or compressed size >= 4GB,
    /// both sizes are promoted into a strictly 16-byte Zip64 extra field and both header
    /// size fields are set to `0xFFFFFFFF`.
    pub fn evaluate_lfh(uncompressed_size: u64, compressed_size: u64) -> LfhZip64Decision {
        let is_zip64 = uncompressed_size >= ZIP64_BYTES_THR || compressed_size >= ZIP64_BYTES_THR;

        if is_zip64 {
            let extra_bytes = Zip64ExtraField::build_lfh(uncompressed_size, compressed_size);
            LfhZip64Decision {
                is_zip64: true,
                uncompressed_size_field: SENTINEL_U32,
                compressed_size_field: SENTINEL_U32,
                version_needed: ZIP64_VERSION_NEEDED,
                extra_field: Some(Zip64ExtraField::Lfh {
                    uncompressed_size,
                    compressed_size,
                }),
                extra_bytes,
            }
        } else {
            LfhZip64Decision {
                is_zip64: false,
                uncompressed_size_field: uncompressed_size as u32,
                compressed_size_field: compressed_size as u32,
                version_needed: 20,
                extra_field: None,
                extra_bytes: Vec::new(),
            }
        }
    }

    /// Evaluates Central Directory File Header (CDFH) promotion criteria.
    ///
    /// In CDFH, fields are individually promoted to 64-bit and set to `0xFFFFFFFF` sentinel
    /// if and only if that specific field exceeds the 4GB threshold.
    pub fn evaluate_cdfh(
        uncompressed_size: u64,
        compressed_size: u64,
        local_offset: u64,
    ) -> CdfhZip64Decision {
        let uncomp_zip64 = uncompressed_size >= ZIP64_BYTES_THR;
        let comp_zip64 = compressed_size >= ZIP64_BYTES_THR;
        let offset_zip64 = local_offset >= ZIP64_BYTES_THR;

        let is_zip64 = uncomp_zip64 || comp_zip64 || offset_zip64;

        if is_zip64 {
            let u_opt = if uncomp_zip64 {
                Some(uncompressed_size)
            } else {
                None
            };
            let c_opt = if comp_zip64 {
                Some(compressed_size)
            } else {
                None
            };
            let o_opt = if offset_zip64 {
                Some(local_offset)
            } else {
                None
            };

            let extra_bytes = Zip64ExtraField::build_cdfh(u_opt, c_opt, o_opt, None);
            CdfhZip64Decision {
                is_zip64: true,
                uncompressed_size_field: if uncomp_zip64 {
                    SENTINEL_U32
                } else {
                    uncompressed_size as u32
                },
                compressed_size_field: if comp_zip64 {
                    SENTINEL_U32
                } else {
                    compressed_size as u32
                },
                local_header_offset_field: if offset_zip64 {
                    SENTINEL_U32
                } else {
                    local_offset as u32
                },
                version_needed: ZIP64_VERSION_NEEDED,
                extra_field: Some(Zip64ExtraField::Cdfh {
                    uncompressed_size: u_opt,
                    compressed_size: c_opt,
                    local_header_offset: o_opt,
                    disk_start_number: None,
                }),
                extra_bytes,
            }
        } else {
            CdfhZip64Decision {
                is_zip64: false,
                uncompressed_size_field: uncompressed_size as u32,
                compressed_size_field: compressed_size as u32,
                local_header_offset_field: local_offset as u32,
                version_needed: 20,
                extra_field: None,
                extra_bytes: Vec::new(),
            }
        }
    }

    /// Evaluates archive-level End of Central Directory (EOCD) promotion criteria.
    ///
    /// Promotes to Zip64 EOCD record and locator if total entries >= 65535,
    /// CD size >= 4GB, or CD starting offset >= 4GB.
    pub fn evaluate_eocd(
        total_entries: u64,
        cd_size: u64,
        cd_offset: u64,
    ) -> EocdZip64Decision {
        let entries_zip64 = total_entries >= ZIP64_ENTRY_THR;
        let size_zip64 = cd_size >= ZIP64_BYTES_THR;
        let offset_zip64 = cd_offset >= ZIP64_BYTES_THR;

        let is_zip64 = entries_zip64 || size_zip64 || offset_zip64;

        if is_zip64 {
            let zip64_eocd_pos = cd_offset + cd_size;
            let eocd_record = Zip64EocdRecord::new(total_entries, cd_size, cd_offset);
            let locator = Zip64EocdLocator::new(zip64_eocd_pos);

            EocdZip64Decision {
                is_zip64: true,
                total_entries_field: if entries_zip64 {
                    SENTINEL_U16
                } else {
                    total_entries as u16
                },
                cd_size_field: if size_zip64 {
                    SENTINEL_U32
                } else {
                    cd_size as u32
                },
                cd_offset_field: if offset_zip64 {
                    SENTINEL_U32
                } else {
                    cd_offset as u32
                },
                zip64_eocd: Some(eocd_record),
                zip64_locator: Some(locator),
            }
        } else {
            EocdZip64Decision {
                is_zip64: false,
                total_entries_field: total_entries as u16,
                cd_size_field: cd_size as u32,
                cd_offset_field: cd_offset as u32,
                zip64_eocd: None,
                zip64_locator: None,
            }
        }
    }

    /// Comprehensive entry decision evaluator for single entry in both LFH and CDFH contexts.
    pub fn evaluate_entry(
        uncompressed_size: u64,
        compressed_size: u64,
        local_offset: u64,
        _entry_index: u64,
    ) -> (LfhZip64Decision, CdfhZip64Decision) {
        (
            Self::evaluate_lfh(uncompressed_size, compressed_size),
            Self::evaluate_cdfh(uncompressed_size, compressed_size, local_offset),
        )
    }
}
