// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zip64 4GB virtual sparse reader and boundary state machine validator (`Zip64VirtualSparseReader`).
//!
//! Simulates multi-gigabyte/terabyte ZIP archives crossing the $2^{32} - 1$ (4GB) boundary with
//! resident memory strictly $\le 4\text{KB}$. Validates 32-bit `0xFFFFFFFF` overflow sentinels
//! and Zip64 Extended Information Extra Field (`0x0001`) state transitions.

use std::io::{self, Read, Seek, SeekFrom};

pub const ZIP_LFH_MAGIC: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];
pub const ZIP_CDH_MAGIC: [u8; 4] = [0x50, 0x4B, 0x01, 0x02];
pub const ZIP64_EOCD_MAGIC: [u8; 4] = [0x50, 0x4B, 0x06, 0x06];
pub const ZIP64_LOCATOR_MAGIC: [u8; 4] = [0x50, 0x4B, 0x06, 0x07];
pub const ZIP_EOCD_MAGIC: [u8; 4] = [0x50, 0x4B, 0x05, 0x06];

pub const ZIP64_OVERFLOW_32: u32 = 0xFFFFFFFF;
pub const ZIP64_OVERFLOW_16: u16 = 0xFFFF;
pub const ZIP64_EXTRA_FIELD_TAG: u16 = 0x0001;
pub const ZIP64_4GB_THRESHOLD: u64 = 1u64 << 32;

/// Errors detected during Zip64 virtual boundary inspection and traversal.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Zip64InspectionError {
    #[error("Unexpected EOF at offset {offset}: expected {expected} bytes, read {read}")]
    UnexpectedEof { offset: u64, expected: usize, read: usize },

    #[error("Signature mismatch at offset {offset}: expected {expected:02X?}, found {found:02X?}")]
    SignatureMismatch { offset: u64, expected: [u8; 4], found: [u8; 4] },

    #[error("Corrupt Zip64 extra field at offset {offset}: {reason}")]
    CorruptExtraField { offset: u64, reason: String },

    #[error("Inconsistent Zip64 state at offset {offset}: sentinel {sentinel:#x}, reason: {reason}")]
    InconsistentState { offset: u64, sentinel: u64, reason: String },

    #[error("I/O error during inspection: {0}")]
    IoError(String),
}

/// Payload type for a virtual sparse segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SegmentData {
    Dense(Vec<u8>),
    Zeroes,
    Pattern(u8),
}

/// A contiguous segment in the virtual archive address space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualSegment {
    pub start_offset: u64,
    pub length: u64,
    pub data: SegmentData,
}

impl VirtualSegment {
    #[inline]
    pub fn end_offset(&self) -> u64 {
        self.start_offset.saturating_add(self.length)
    }

    pub fn read_at(&self, offset: u64, buf: &mut [u8]) -> usize {
        if offset < self.start_offset || offset >= self.end_offset() || buf.is_empty() {
            return 0;
        }
        let rel_offset = offset - self.start_offset;
        let available = self.length - rel_offset;
        let to_read = (buf.len() as u64).min(available) as usize;

        match &self.data {
            SegmentData::Dense(dense) => {
                let idx = rel_offset as usize;
                let end = (idx + to_read).min(dense.len());
                if idx < dense.len() {
                    let n = end - idx;
                    buf[..n].copy_from_slice(&dense[idx..end]);
                    n
                } else {
                    0
                }
            }
            SegmentData::Zeroes => {
                buf[..to_read].fill(0);
                to_read
            }
            SegmentData::Pattern(b) => {
                buf[..to_read].fill(*b);
                to_read
            }
        }
    }
}

/// Zero-allocation virtual sparse reader that emulates multi-gigabyte Zip64 archives.
#[derive(Debug, Clone)]
pub struct Zip64VirtualSparseReader {
    segments: Vec<VirtualSegment>,
    total_length: u64,
    pointer: u64,
}

impl Zip64VirtualSparseReader {
    pub fn new(mut segments: Vec<VirtualSegment>) -> Self {
        segments.sort_by_key(|s| s.start_offset);
        let total_length = segments.iter().map(|s| s.end_offset()).max().unwrap_or(0);
        Self { segments, total_length, pointer: 0 }
    }

    pub fn empty() -> Self {
        Self { segments: Vec::new(), total_length: 0, pointer: 0 }
    }

    pub fn add_dense_segment(&mut self, offset: u64, data: Vec<u8>) {
        let length = data.len() as u64;
        self.segments.push(VirtualSegment { start_offset: offset, length, data: SegmentData::Dense(data) });
        self.segments.sort_by_key(|s| s.start_offset);
        self.total_length = self.total_length.max(offset.saturating_add(length));
    }

    #[inline]
    pub fn total_virtual_length(&self) -> u64 {
        self.total_length
    }

    pub fn resident_memory_footprint(&self) -> usize {
        let mut bytes = std::mem::size_of::<Self>();
        for seg in &self.segments {
            bytes += std::mem::size_of::<VirtualSegment>();
            if let SegmentData::Dense(d) = &seg.data {
                bytes += d.capacity();
            }
        }
        bytes
    }

    pub fn new_5gb_overflow_archive() -> Self {
        let mut builder = Zip64ArchiveBuilder::new();
        builder.add_file("huge_data.bin", 4_831_838_208, 0x12345678);
        builder.add_file("boundary_post.dat", 100 * 1024 * 1024, 0x87654321);
        builder.build()
    }
}

impl Seek for Zip64VirtualSparseReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(off) => off as i64,
            SeekFrom::End(off) => self.total_length as i64 + off,
            SeekFrom::Current(off) => self.pointer as i64 + off,
        };
        if new_pos < 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Seek before origin"));
        }
        self.pointer = new_pos as u64;
        Ok(self.pointer)
    }
}

impl Read for Zip64VirtualSparseReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pointer >= self.total_length || buf.is_empty() {
            return Ok(0);
        }
        let max_possible = (self.total_length - self.pointer).min(buf.len() as u64) as usize;
        let mut target_buf = &mut buf[..max_possible];
        let mut total_read = 0;

        while !target_buf.is_empty() && self.pointer < self.total_length {
            let seg_opt = self.segments.iter().find(|s| self.pointer >= s.start_offset && self.pointer < s.end_offset());
            if let Some(seg) = seg_opt {
                let n = seg.read_at(self.pointer, target_buf);
                if n == 0 { break; }
                self.pointer += n as u64;
                total_read += n;
                target_buf = &mut target_buf[n..];
            } else {
                let next_start = self.segments.iter().map(|s| s.start_offset).filter(|&s| s > self.pointer).min().unwrap_or(self.total_length);
                let gap_len = (next_start - self.pointer).min(target_buf.len() as u64) as usize;
                target_buf[..gap_len].fill(0);
                self.pointer += gap_len as u64;
                total_read += gap_len;
                target_buf = &mut target_buf[gap_len..];
            }
        }
        Ok(total_read)
    }
}

/// Helper for constructing synthetic Zip64 archives in virtual sparse space.
#[derive(Default)]
pub struct Zip64ArchiveBuilder {
    entries: Vec<(String, u64, u32)>,
}

impl Zip64ArchiveBuilder {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn add_file(&mut self, name: &str, uncompressed_size: u64, crc32: u32) {
        self.entries.push((name.to_string(), uncompressed_size, crc32));
    }

    pub fn build(self) -> Zip64VirtualSparseReader {
        let mut segments = Vec::new();
        let mut offset = 0u64;
        let mut cdh_entries = Vec::new();

        for (name, size, crc) in &self.entries {
            let lfh_offset = offset;
            let name_bytes = name.as_bytes();
            let is_z64 = *size >= ZIP64_OVERFLOW_32 as u64;

            let mut extra = Vec::new();
            if is_z64 {
                extra.extend_from_slice(&ZIP64_EXTRA_FIELD_TAG.to_le_bytes());
                extra.extend_from_slice(&16u16.to_le_bytes());
                extra.extend_from_slice(&size.to_le_bytes());
                extra.extend_from_slice(&size.to_le_bytes());
            }

            let mut lfh = Vec::with_capacity(30 + name_bytes.len() + extra.len());
            lfh.extend_from_slice(&ZIP_LFH_MAGIC);
            lfh.extend_from_slice(&45u16.to_le_bytes()); // version
            lfh.extend_from_slice(&0u16.to_le_bytes()); // flags
            lfh.extend_from_slice(&0u16.to_le_bytes()); // stored
            lfh.extend_from_slice(&0x4521u16.to_le_bytes()); // time
            lfh.extend_from_slice(&0x5467u16.to_le_bytes()); // date
            lfh.extend_from_slice(&crc.to_le_bytes());
            let sz32 = if is_z64 { ZIP64_OVERFLOW_32 } else { *size as u32 };
            lfh.extend_from_slice(&sz32.to_le_bytes());
            lfh.extend_from_slice(&sz32.to_le_bytes());
            lfh.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            lfh.extend_from_slice(&(extra.len() as u16).to_le_bytes());
            lfh.extend_from_slice(name_bytes);
            lfh.extend_from_slice(&extra);

            let lfh_len = lfh.len() as u64;
            segments.push(VirtualSegment { start_offset: lfh_offset, length: lfh_len, data: SegmentData::Dense(lfh) });
            offset += lfh_len;

            if *size > 0 {
                segments.push(VirtualSegment { start_offset: offset, length: *size, data: SegmentData::Zeroes });
                offset += *size;
            }
            cdh_entries.push((name.clone(), *size, *crc, lfh_offset));
        }

        let cd_start = offset;
        let mut cd_bytes = Vec::new();

        for (name, size, crc, lfh_off) in &cdh_entries {
            let name_bytes = name.as_bytes();
            let is_sz64 = *size >= ZIP64_OVERFLOW_32 as u64;
            let is_off64 = *lfh_off >= ZIP64_OVERFLOW_32 as u64;

            let mut extra = Vec::new();
            if is_sz64 || is_off64 {
                let mut data = Vec::new();
                if is_sz64 { data.extend_from_slice(&size.to_le_bytes()); data.extend_from_slice(&size.to_le_bytes()); }
                if is_off64 { data.extend_from_slice(&lfh_off.to_le_bytes()); }
                extra.extend_from_slice(&ZIP64_EXTRA_FIELD_TAG.to_le_bytes());
                extra.extend_from_slice(&(data.len() as u16).to_le_bytes());
                extra.extend_from_slice(&data);
            }

            let mut cdh = Vec::with_capacity(46 + name_bytes.len() + extra.len());
            cdh.extend_from_slice(&ZIP_CDH_MAGIC);
            cdh.extend_from_slice(&45u16.to_le_bytes());
            cdh.extend_from_slice(&45u16.to_le_bytes());
            cdh.extend_from_slice(&0u16.to_le_bytes());
            cdh.extend_from_slice(&0u16.to_le_bytes());
            cdh.extend_from_slice(&0x4521u16.to_le_bytes());
            cdh.extend_from_slice(&0x5467u16.to_le_bytes());
            cdh.extend_from_slice(&crc.to_le_bytes());
            let sz32 = if is_sz64 { ZIP64_OVERFLOW_32 } else { *size as u32 };
            cdh.extend_from_slice(&sz32.to_le_bytes());
            cdh.extend_from_slice(&sz32.to_le_bytes());
            cdh.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            cdh.extend_from_slice(&(extra.len() as u16).to_le_bytes());
            cdh.extend_from_slice(&0u16.to_le_bytes()); // comment len
            cdh.extend_from_slice(&0u16.to_le_bytes()); // disk start
            cdh.extend_from_slice(&0u16.to_le_bytes()); // internal
            cdh.extend_from_slice(&0x81A40000u32.to_le_bytes()); // external
            let off32 = if is_off64 { ZIP64_OVERFLOW_32 } else { *lfh_off as u32 };
            cdh.extend_from_slice(&off32.to_le_bytes());
            cdh.extend_from_slice(name_bytes);
            cdh.extend_from_slice(&extra);
            cd_bytes.extend_from_slice(&cdh);
        }

        let cd_size = cd_bytes.len() as u64;
        segments.push(VirtualSegment { start_offset: cd_start, length: cd_size, data: SegmentData::Dense(cd_bytes) });
        offset += cd_size;

        // Zip64 EOCD
        let z64_eocd_off = offset;
        let mut z64_eocd = Vec::with_capacity(56);
        z64_eocd.extend_from_slice(&ZIP64_EOCD_MAGIC);
        z64_eocd.extend_from_slice(&44u64.to_le_bytes());
        z64_eocd.extend_from_slice(&45u16.to_le_bytes());
        z64_eocd.extend_from_slice(&45u16.to_le_bytes());
        z64_eocd.extend_from_slice(&0u32.to_le_bytes());
        z64_eocd.extend_from_slice(&0u32.to_le_bytes());
        let entries_count = self.entries.len() as u64;
        z64_eocd.extend_from_slice(&entries_count.to_le_bytes());
        z64_eocd.extend_from_slice(&entries_count.to_le_bytes());
        z64_eocd.extend_from_slice(&cd_size.to_le_bytes());
        z64_eocd.extend_from_slice(&cd_start.to_le_bytes());
        let z64_len = z64_eocd.len() as u64;
        segments.push(VirtualSegment { start_offset: z64_eocd_off, length: z64_len, data: SegmentData::Dense(z64_eocd) });
        offset += z64_len;

        // Locator
        let loc_off = offset;
        let mut loc = Vec::with_capacity(20);
        loc.extend_from_slice(&ZIP64_LOCATOR_MAGIC);
        loc.extend_from_slice(&0u32.to_le_bytes());
        loc.extend_from_slice(&z64_eocd_off.to_le_bytes());
        loc.extend_from_slice(&1u32.to_le_bytes());
        let loc_len = loc.len() as u64;
        segments.push(VirtualSegment { start_offset: loc_off, length: loc_len, data: SegmentData::Dense(loc) });
        offset += loc_len;

        // EOCD
        let mut eocd = Vec::with_capacity(22);
        eocd.extend_from_slice(&ZIP_EOCD_MAGIC);
        eocd.extend_from_slice(&0u16.to_le_bytes());
        eocd.extend_from_slice(&0u16.to_le_bytes());
        let count16 = entries_count.min(ZIP64_OVERFLOW_16 as u64) as u16;
        eocd.extend_from_slice(&count16.to_le_bytes());
        eocd.extend_from_slice(&count16.to_le_bytes());
        eocd.extend_from_slice(&ZIP64_OVERFLOW_32.to_le_bytes());
        eocd.extend_from_slice(&ZIP64_OVERFLOW_32.to_le_bytes());
        eocd.extend_from_slice(&0u16.to_le_bytes());
        segments.push(VirtualSegment { start_offset: offset, length: eocd.len() as u64, data: SegmentData::Dense(eocd) });

        Zip64VirtualSparseReader::new(segments)
    }
}

/// Parsed metadata from headers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zip64LocalHeaderInfo {
    pub offset: u64,
    pub filename: String,
    pub declared_uncompressed_size: u64,
    pub declared_compressed_size: u64,
    pub crc32: u32,
    pub is_zip64_extended: bool,
    pub raw_header_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zip64CentralHeaderInfo {
    pub offset: u64,
    pub filename: String,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub local_header_offset: u64,
    pub is_zip64_extended: bool,
    pub has_offset_overflow: bool,
    pub has_size_overflow: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zip64EocdInfo {
    pub offset: u64,
    pub total_entries: u64,
    pub central_directory_size: u64,
    pub central_directory_offset: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zip64VerificationReport {
    pub total_virtual_bytes: u64,
    pub resident_memory_bytes: usize,
    pub local_headers_count: usize,
    pub central_headers_count: usize,
    pub zip64_entries_detected: usize,
    pub boundary_4gb_crossed: bool,
    pub valid_state_machine: bool,
}

/// Tuple holding parsed values from a Zip64 extra field: (found, uncompressed_size, compressed_size, local_header_offset).
pub type ParsedZip64ExtraField = (bool, Option<u64>, Option<u64>, Option<u64>);

/// Shared parser for Zip64 Extra Field (`0x0001`).
fn parse_extra_field_zip64(
    extra: &[u8],
    need_uncomp: bool,
    need_comp: bool,
    need_offset: bool,
) -> Result<ParsedZip64ExtraField, Zip64InspectionError> {
    let mut pos = 0;
    while pos + 4 <= extra.len() {
        let tag = u16::from_le_bytes([extra[pos], extra[pos + 1]]);
        let size = u16::from_le_bytes([extra[pos + 2], extra[pos + 3]]) as usize;
        pos += 4;
        if pos + size > extra.len() {
            return Err(Zip64InspectionError::CorruptExtraField { offset: 0, reason: "Extra field exceeds buffer".into() });
        }
        if tag == ZIP64_EXTRA_FIELD_TAG {
            let mut dp = pos;
            let uncomp = if need_uncomp && dp + 8 <= pos + size {
                let v = u64::from_le_bytes(extra[dp..dp + 8].try_into().unwrap());
                dp += 8;
                Some(v)
            } else { None };

            let comp = if need_comp && dp + 8 <= pos + size {
                let v = u64::from_le_bytes(extra[dp..dp + 8].try_into().unwrap());
                dp += 8;
                Some(v)
            } else { None };

            let offset = if need_offset && dp + 8 <= pos + size {
                let v = u64::from_le_bytes(extra[dp..dp + 8].try_into().unwrap());
                Some(v)
            } else { None };

            return Ok((true, uncomp, comp, offset));
        }
        pos += size;
    }
    Ok((false, None, None, None))
}

pub struct Zip64HeaderInspector;

impl Zip64HeaderInspector {
    pub fn parse_local_header<R: Read + Seek>(reader: &mut R, offset: u64) -> Result<Zip64LocalHeaderInfo, Zip64InspectionError> {
        reader.seek(SeekFrom::Start(offset)).map_err(|e| Zip64InspectionError::IoError(e.to_string()))?;
        let mut fixed = [0u8; 30];
        reader.read_exact(&mut fixed).map_err(|_| Zip64InspectionError::UnexpectedEof { offset, expected: 30, read: 0 })?;
        if fixed[0..4] != ZIP_LFH_MAGIC {
            return Err(Zip64InspectionError::SignatureMismatch { offset, expected: ZIP_LFH_MAGIC, found: [fixed[0], fixed[1], fixed[2], fixed[3]] });
        }
        let crc32 = u32::from_le_bytes(fixed[14..18].try_into().unwrap());
        let comp32 = u32::from_le_bytes(fixed[18..22].try_into().unwrap());
        let uncomp32 = u32::from_le_bytes(fixed[22..26].try_into().unwrap());
        let name_len = u16::from_le_bytes(fixed[26..28].try_into().unwrap()) as usize;
        let extra_len = u16::from_le_bytes(fixed[28..30].try_into().unwrap()) as usize;

        let mut name_buf = vec![0u8; name_len];
        reader.read_exact(&mut name_buf).map_err(|_| Zip64InspectionError::UnexpectedEof { offset: offset + 30, expected: name_len, read: 0 })?;
        let filename = String::from_utf8_lossy(&name_buf).to_string();

        let mut extra_buf = vec![0u8; extra_len];
        reader.read_exact(&mut extra_buf).map_err(|_| Zip64InspectionError::UnexpectedEof { offset: offset + 30 + name_len as u64, expected: extra_len, read: 0 })?;

        let (is_z64, z_uncomp, z_comp, _) = parse_extra_field_zip64(&extra_buf, uncomp32 == ZIP64_OVERFLOW_32, comp32 == ZIP64_OVERFLOW_32, false)?;
        if (uncomp32 == ZIP64_OVERFLOW_32 || comp32 == ZIP64_OVERFLOW_32) && !is_z64 {
            return Err(Zip64InspectionError::InconsistentState { offset, sentinel: ZIP64_OVERFLOW_32 as u64, reason: "0xFFFFFFFF sentinel without extra field".into() });
        }

        Ok(Zip64LocalHeaderInfo {
            offset,
            filename,
            declared_uncompressed_size: z_uncomp.unwrap_or(uncomp32 as u64),
            declared_compressed_size: z_comp.unwrap_or(comp32 as u64),
            crc32,
            is_zip64_extended: is_z64,
            raw_header_size: 30 + name_len + extra_len,
        })
    }

    pub fn parse_central_header<R: Read + Seek>(reader: &mut R, offset: u64) -> Result<Zip64CentralHeaderInfo, Zip64InspectionError> {
        reader.seek(SeekFrom::Start(offset)).map_err(|e| Zip64InspectionError::IoError(e.to_string()))?;
        let mut fixed = [0u8; 46];
        reader.read_exact(&mut fixed).map_err(|_| Zip64InspectionError::UnexpectedEof { offset, expected: 46, read: 0 })?;
        if fixed[0..4] != ZIP_CDH_MAGIC {
            return Err(Zip64InspectionError::SignatureMismatch { offset, expected: ZIP_CDH_MAGIC, found: [fixed[0], fixed[1], fixed[2], fixed[3]] });
        }
        let comp32 = u32::from_le_bytes(fixed[20..24].try_into().unwrap());
        let uncomp32 = u32::from_le_bytes(fixed[24..28].try_into().unwrap());
        let name_len = u16::from_le_bytes(fixed[28..30].try_into().unwrap()) as usize;
        let extra_len = u16::from_le_bytes(fixed[30..32].try_into().unwrap()) as usize;
        let comment_len = u16::from_le_bytes(fixed[32..34].try_into().unwrap()) as usize;
        let rel_off32 = u32::from_le_bytes(fixed[42..46].try_into().unwrap());

        let mut name_buf = vec![0u8; name_len];
        reader.read_exact(&mut name_buf).map_err(|_| Zip64InspectionError::UnexpectedEof { offset: offset + 46, expected: name_len, read: 0 })?;
        let filename = String::from_utf8_lossy(&name_buf).to_string();

        let mut extra_buf = vec![0u8; extra_len];
        reader.read_exact(&mut extra_buf).map_err(|_| Zip64InspectionError::UnexpectedEof { offset: offset + 46 + name_len as u64, expected: extra_len, read: 0 })?;

        let has_sz = uncomp32 == ZIP64_OVERFLOW_32 || comp32 == ZIP64_OVERFLOW_32;
        let has_off = rel_off32 == ZIP64_OVERFLOW_32;
        let (is_z64, z_uncomp, z_comp, z_off) = parse_extra_field_zip64(&extra_buf, uncomp32 == ZIP64_OVERFLOW_32, comp32 == ZIP64_OVERFLOW_32, has_off)?;
        if (has_sz || has_off) && !is_z64 {
            return Err(Zip64InspectionError::InconsistentState { offset, sentinel: ZIP64_OVERFLOW_32 as u64, reason: "0xFFFFFFFF sentinel without extra field".into() });
        }
        if comment_len > 0 { let mut c = vec![0u8; comment_len]; let _ = reader.read_exact(&mut c); }

        Ok(Zip64CentralHeaderInfo {
            offset,
            filename,
            uncompressed_size: z_uncomp.unwrap_or(uncomp32 as u64),
            compressed_size: z_comp.unwrap_or(comp32 as u64),
            local_header_offset: z_off.unwrap_or(rel_off32 as u64),
            is_zip64_extended: is_z64,
            has_offset_overflow: has_off,
            has_size_overflow: has_sz,
        })
    }

    pub fn verify_archive(reader: &mut Zip64VirtualSparseReader) -> Result<Zip64VerificationReport, Zip64InspectionError> {
        let total_virtual_bytes = reader.total_virtual_length();
        let resident_memory_bytes = reader.resident_memory_footprint();

        let scan_start = total_virtual_bytes.saturating_sub(128);
        reader.seek(SeekFrom::Start(scan_start)).map_err(|e| Zip64InspectionError::IoError(e.to_string()))?;
        let mut tail_buf = vec![0u8; (total_virtual_bytes - scan_start) as usize];
        reader.read_exact(&mut tail_buf).map_err(|_| Zip64InspectionError::UnexpectedEof { offset: scan_start, expected: tail_buf.len(), read: 0 })?;

        let mut locator_rel = None;
        for i in 0..tail_buf.len().saturating_sub(19) {
            if tail_buf[i..i + 4] == ZIP64_LOCATOR_MAGIC {
                locator_rel = Some(i);
                break;
            }
        }
        let loc_idx = locator_rel.ok_or(Zip64InspectionError::SignatureMismatch { offset: scan_start, expected: ZIP64_LOCATOR_MAGIC, found: [0; 4] })?;
        let z64_eocd_off = u64::from_le_bytes(tail_buf[loc_idx + 8..loc_idx + 16].try_into().unwrap());

        reader.seek(SeekFrom::Start(z64_eocd_off)).map_err(|e| Zip64InspectionError::IoError(e.to_string()))?;
        let mut eocd_buf = [0u8; 56];
        reader.read_exact(&mut eocd_buf).map_err(|_| Zip64InspectionError::UnexpectedEof { offset: z64_eocd_off, expected: 56, read: 0 })?;

        let total_entries = u64::from_le_bytes(eocd_buf[32..40].try_into().unwrap());
        let cd_size = u64::from_le_bytes(eocd_buf[40..48].try_into().unwrap());
        let cd_offset = u64::from_le_bytes(eocd_buf[48..56].try_into().unwrap());

        let mut cur_cd = cd_offset;
        let cd_end = cd_offset + cd_size;
        let mut central_headers = Vec::new();
        let mut z64_count = 0;

        while cur_cd < cd_end {
            let cdh = Self::parse_central_header(reader, cur_cd)?;
            if cdh.is_zip64_extended { z64_count += 1; }
            cur_cd = reader.stream_position().map_err(|e| Zip64InspectionError::IoError(e.to_string()))?;
            central_headers.push(cdh);
        }

        let mut local_headers = Vec::new();
        for cdh in &central_headers {
            let lfh = Self::parse_local_header(reader, cdh.local_header_offset)?;
            assert_eq!(lfh.filename, cdh.filename);
            local_headers.push(lfh);
        }

        Ok(Zip64VerificationReport {
            total_virtual_bytes,
            resident_memory_bytes,
            local_headers_count: local_headers.len(),
            central_headers_count: central_headers.len(),
            zip64_entries_detected: z64_count,
            boundary_4gb_crossed: total_virtual_bytes > ZIP64_4GB_THRESHOLD,
            valid_state_machine: central_headers.len() == total_entries as usize,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtual_sparse_reader_memory_footprint_le_4kb() {
        let archive = Zip64VirtualSparseReader::new_5gb_overflow_archive();
        assert!(archive.total_virtual_length() > 4_500_000_000);
        let resident = archive.resident_memory_footprint();
        assert!(resident <= 4096, "Resident memory {} > 4KB", resident);
    }

    #[test]
    fn test_zip64_virtual_sparse_reader_stream_seek_and_read() {
        let mut archive = Zip64VirtualSparseReader::new_5gb_overflow_archive();
        let mut magic = [0u8; 4];
        archive.read_exact(&mut magic).unwrap();
        assert_eq!(magic, ZIP_LFH_MAGIC);

        archive.seek(SeekFrom::Start(4_000_000_000)).unwrap();
        let mut buf = [0xFFu8; 16];
        archive.read_exact(&mut buf).unwrap();
        assert_eq!(buf, [0u8; 16]);

        archive.seek(SeekFrom::End(-22)).unwrap();
        let mut eocd_sig = [0u8; 4];
        archive.read_exact(&mut eocd_sig).unwrap();
        assert_eq!(eocd_sig, ZIP_EOCD_MAGIC);
    }

    #[test]
    fn test_zip64_state_machine_and_overflow_verification() {
        let mut archive = Zip64VirtualSparseReader::new_5gb_overflow_archive();
        let report = Zip64HeaderInspector::verify_archive(&mut archive).unwrap();
        assert!(report.boundary_4gb_crossed);
        assert_eq!(report.local_headers_count, 2);
        assert_eq!(report.central_headers_count, 2);
        assert_eq!(report.zip64_entries_detected, 2);
        assert!(report.valid_state_machine);
        assert!(report.resident_memory_bytes <= 4096);
    }

    #[test]
    fn test_inconsistent_zip64_state_detection() {
        let mut reader = Zip64VirtualSparseReader::empty();
        let mut bad_lfh = vec![0u8; 30];
        bad_lfh[0..4].copy_from_slice(&ZIP_LFH_MAGIC);
        bad_lfh[22..26].copy_from_slice(&ZIP64_OVERFLOW_32.to_le_bytes());
        bad_lfh[28..30].copy_from_slice(&0u16.to_le_bytes());
        reader.add_dense_segment(0, bad_lfh);

        let err = Zip64HeaderInspector::parse_local_header(&mut reader, 0).unwrap_err();
        match err {
            Zip64InspectionError::InconsistentState { sentinel, .. } => {
                assert_eq!(sentinel, 0xFFFFFFFF);
            }
            _ => panic!("Expected InconsistentState, got {:?}", err),
        }
    }
}
