// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! APFS and POSIX native sparse file hole-punching writer and extent state machine.
//!
//! Provides zero-block detection, automated `lseek` hole punching with 16KB Apple Silicon
//! block alignment, physical extent coalescing, and trailing hole finalization via `ftruncate`.

use crate::archive::unified::entry::sparse::{coalesce_sparse_extents, SparseExtent};
use crate::fs::apfs::APPLE_SILICON_PAGE_SIZE;

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// Fast zero-block detector utilizing native word alignment.
#[inline]
pub fn is_zero_block(buf: &[u8]) -> bool {
    // SAFETY: align_to divides the slice into unaligned prefix, aligned u64 chunks, and unaligned suffix
    let (prefix, chunks, suffix) = unsafe { buf.align_to::<u64>() };
    if prefix.iter().any(|&b| b != 0) {
        return false;
    }
    if chunks.iter().any(|&w| w != 0) {
        return false;
    }
    suffix.iter().all(|&b| b == 0)
}

/// Sparse file writer state machine with automated hole punching and APFS extent tracking.
pub struct SparseFileWriter {
    file: File,
    block_size: usize,
    current_offset: u64,
    max_offset: u64,
    target_size: Option<u64>,
    extents: Vec<SparseExtent>,
    buffer: Vec<u8>,
    hole_pending: bool,
}

impl SparseFileWriter {
    /// Creates and opens a new file at `path` for sparse writing (creating or truncating).
    pub fn create(path: &Path) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        Ok(Self::from_file(file))
    }

    /// Wraps an existing open `File` handle into a `SparseFileWriter`.
    pub fn from_file(file: File) -> Self {
        Self {
            file,
            block_size: APPLE_SILICON_PAGE_SIZE,
            current_offset: 0,
            max_offset: 0,
            target_size: None,
            extents: Vec::new(),
            buffer: Vec::with_capacity(APPLE_SILICON_PAGE_SIZE),
            hole_pending: false,
        }
    }

    /// Configures custom block size alignment (defaults to 16KB Apple Silicon page size).
    #[must_use]
    pub fn with_block_size(mut self, size: usize) -> Self {
        self.block_size = size.max(512);
        self
    }

    /// Sets explicit target logical file size to be finalized in `finish()`.
    pub fn set_target_size(&mut self, size: u64) {
        self.target_size = Some(size);
        self.max_offset = self.max_offset.max(size);
    }

    /// Returns explicit target logical size if specified.
    #[inline]
    pub fn target_size(&self) -> Option<u64> {
        self.target_size
    }

    /// Returns current logical file size (maximum written or seeked offset).
    #[inline]
    pub fn logical_size(&self) -> u64 {
        self.target_size.unwrap_or(self.max_offset)
    }

    /// Queries the physical disk allocation size in bytes (via POSIX `st_blocks * 512`).
    pub fn physical_bytes(&self) -> std::io::Result<u64> {
        let meta = self.file.metadata()?;
        Ok(meta.blocks().saturating_mul(512))
    }

    /// Returns the recorded non-zero data extents.
    pub fn extents(&self) -> &[SparseExtent] {
        &self.extents
    }

    /// Writes data directly at an explicit offset, punching holes as necessary.
    pub fn write_extent(&mut self, offset: u64, data: &[u8]) -> std::io::Result<()> {
        self.seek(SeekFrom::Start(offset))?;
        self.write_all(data)?;
        self.flush()?;
        Ok(())
    }

    /// Reads sparse extents from an input stream and writes them directly to the destination.
    pub fn write_sparse_extents<R: Read>(
        &mut self,
        reader: &mut R,
        extents: &[SparseExtent],
        total_size: u64,
    ) -> std::io::Result<()> {
        self.set_target_size(total_size);
        let mut copy_buf = vec![0u8; self.block_size];

        for extent in extents {
            self.seek(SeekFrom::Start(extent.offset))?;
            let mut remaining = extent.length;

            while remaining > 0 {
                let to_read = (remaining as usize).min(copy_buf.len());
                reader.read_exact(&mut copy_buf[..to_read])?;
                self.write_all(&copy_buf[..to_read])?;
                remaining -= to_read as u64;
            }
        }

        self.flush()?;
        Ok(())
    }

    /// Appends a non-zero extent to the internal tracking list.
    fn record_extent(&mut self, extent: SparseExtent) {
        if extent.is_empty() {
            return;
        }
        if let Some(last) = self.extents.last_mut() {
            if let Some(merged) = last.coalesce_with(&extent) {
                *last = merged;
                return;
            }
        }
        self.extents.push(extent);
    }

    /// Processes a single block: punches holes if all zero, or writes non-zero data to disk.
    fn process_block(&mut self, block: &[u8]) -> std::io::Result<()> {
        if block.is_empty() {
            return Ok(());
        }

        if is_zero_block(block) {
            self.hole_pending = true;
            self.current_offset = self.current_offset.saturating_add(block.len() as u64);
            self.max_offset = self.max_offset.max(self.current_offset);
        } else {
            if self.hole_pending {
                self.file.seek(SeekFrom::Start(self.current_offset))?;
                self.hole_pending = false;
            }
            let extent = SparseExtent::new(self.current_offset, block.len() as u64);
            self.record_extent(extent);
            self.file.write_all(block)?;
            self.current_offset = self.current_offset.saturating_add(block.len() as u64);
            self.max_offset = self.max_offset.max(self.current_offset);
        }

        Ok(())
    }

    /// Finalizes the sparse file by closing trailing holes with `ftruncate` and syncing metadata.
    pub fn finish(mut self) -> std::io::Result<u64> {
        self.flush()?;
        let final_len = self.target_size.unwrap_or(self.max_offset);
        self.file.set_len(final_len)?;
        coalesce_sparse_extents(&mut self.extents);
        Ok(final_len)
    }
}

impl Write for SparseFileWriter {
    fn write(&mut self, mut buf: &[u8]) -> std::io::Result<usize> {
        let total_written = buf.len();

        // 1. Fill partial buffer if present
        if !self.buffer.is_empty() {
            let needed = self.block_size.saturating_sub(self.buffer.len());
            let take_len = needed.min(buf.len());
            self.buffer.extend_from_slice(&buf[..take_len]);
            buf = &buf[take_len..];

            if self.buffer.len() == self.block_size {
                let full_block = std::mem::take(&mut self.buffer);
                self.process_block(&full_block)?;
            }
        }

        // 2. Process complete blocks directly
        while buf.len() >= self.block_size {
            let (block, rest) = buf.split_at(self.block_size);
            self.process_block(block)?;
            buf = rest;
        }

        // 3. Stash leftover trailing bytes
        if !buf.is_empty() {
            self.buffer.extend_from_slice(buf);
        }

        Ok(total_written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if !self.buffer.is_empty() {
            let remaining = std::mem::take(&mut self.buffer);
            self.process_block(&remaining)?;
        }
        self.file.flush()?;
        Ok(())
    }
}

impl Seek for SparseFileWriter {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.flush()?;

        let new_offset = match pos {
            SeekFrom::Start(n) => n,
            SeekFrom::Current(delta) => {
                let cur = self.current_offset as i64;
                let target = cur.checked_add(delta).ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "Seek offset overflow")
                })?;
                if target < 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Seek to negative position",
                    ));
                }
                target as u64
            }
            SeekFrom::End(delta) => {
                let logical_len = self.target_size.unwrap_or(self.max_offset) as i64;
                let target = logical_len.checked_add(delta).ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "Seek offset overflow")
                })?;
                if target < 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Seek to negative position",
                    ));
                }
                target as u64
            }
        };

        self.current_offset = new_offset;
        self.max_offset = self.max_offset.max(new_offset);
        self.hole_pending = true;
        Ok(new_offset)
    }
}

/// Scans a seekable reader to detect non-zero sparse data extents.
pub fn detect_sparse_extents_from_reader<R: Read + Seek>(
    reader: &mut R,
    total_size: u64,
    block_size: usize,
) -> std::io::Result<Vec<SparseExtent>> {
    let mut extents = Vec::new();
    let mut buffer = vec![0u8; block_size.max(512)];
    let mut current_offset: u64 = 0;
    let mut in_data_extent = false;
    let mut extent_start: u64 = 0;

    reader.seek(SeekFrom::Start(0))?;

    while current_offset < total_size {
        let remaining = (total_size - current_offset) as usize;
        let to_read = remaining.min(buffer.len());
        reader.read_exact(&mut buffer[..to_read])?;

        let is_zero = is_zero_block(&buffer[..to_read]);

        if is_zero {
            if in_data_extent {
                extents.push(SparseExtent::new(
                    extent_start,
                    current_offset.saturating_sub(extent_start),
                ));
                in_data_extent = false;
            }
        } else if !in_data_extent {
            in_data_extent = true;
            extent_start = current_offset;
        }

        current_offset += to_read as u64;
    }

    if in_data_extent {
        extents.push(SparseExtent::new(
            extent_start,
            total_size.saturating_sub(extent_start),
        ));
    }

    coalesce_sparse_extents(&mut extents);
    Ok(extents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tempfile::tempdir;

    #[test]
    fn test_zero_block_detector() {
        let zero_small = [0u8; 12];
        assert!(is_zero_block(&zero_small));

        let zero_16k = vec![0u8; 16384];
        assert!(is_zero_block(&zero_16k));

        let mut non_zero = vec![0u8; 16384];
        non_zero[8192] = 0x01;
        assert!(!is_zero_block(&non_zero));

        let mut non_zero_tail = vec![0u8; 17];
        non_zero_tail[16] = 0xFF;
        assert!(!is_zero_block(&non_zero_tail));
    }

    #[test]
    fn test_sparse_writer_hole_punching() {
        let tmp = tempdir().unwrap();
        let file_path = tmp.path().join("sparse_test.bin");

        let mut writer = SparseFileWriter::create(&file_path)
            .unwrap()
            .with_block_size(16384);

        // 1. Write 32KB zeros (hole)
        let zeros = vec![0u8; 32768];
        writer.write_all(&zeros).unwrap();

        // 2. Write 16KB data
        let data = vec![0xAAu8; 16384];
        writer.write_all(&data).unwrap();

        // 3. Write 32KB zeros (tail hole)
        writer.write_all(&zeros).unwrap();

        let final_size = writer.finish().unwrap();
        assert_eq!(final_size, 81920);

        // Read back data to verify integrity
        let read_back = std::fs::read(&file_path).unwrap();
        assert_eq!(read_back.len(), 81920);
        assert_eq!(&read_back[0..32768], &zeros[..]);
        assert_eq!(&read_back[32768..49152], &data[..]);
        assert_eq!(&read_back[49152..81920], &zeros[..]);
    }

    #[test]
    fn test_detect_sparse_extents_from_reader() {
        let mut data = vec![0u8; 64 * 1024];
        // Non-zero extent at 16KB..32KB
        for b in &mut data[16384..32768] {
            *b = 0x55;
        }

        let mut cursor = Cursor::new(data);
        let extents = detect_sparse_extents_from_reader(&mut cursor, 64 * 1024, 16384).unwrap();

        assert_eq!(extents.len(), 1);
        assert_eq!(extents[0].offset, 16384);
        assert_eq!(extents[0].length, 16384);
    }
}
