// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI Memory-Mapped (Mmap) Zero-Copy Data Stream & Kernel Advice Scaffolding.

use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::sync::Arc;
use super::types::TTZipError;

/// Kernel memory access advice exposed via UniFFI to optimize OS page cache behavior.
#[derive(Copy, Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum UniFFIMmapAdvice {
    Normal,
    Sequential,
    Random,
    WillNeed,
    DontNeed,
}

/// Aggregated memory map diagnostics and page alignment statistics.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIMmapStats {
    pub file_size: u64,
    pub mapped_size: u64,
    pub is_empty: bool,
    pub page_size: u32,
    pub is_readonly: bool,
}

/// Zero-copy slice payload descriptor across FFI boundary.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIMmapSlice {
    pub offset: u64,
    pub length: u64,
    pub data: Vec<u8>,
}

struct SafeMmapAllocation {
    ptr: *const u8,
    len: usize,
}

unsafe impl Send for SafeMmapAllocation {}
unsafe impl Sync for SafeMmapAllocation {}

impl Drop for SafeMmapAllocation {
    fn drop(&mut self) {
        if !self.ptr.is_null() && self.len > 0 {
            unsafe {
                libc::munmap(self.ptr as *mut libc::c_void, self.len);
            }
        }
    }
}

/// High-performance zero-copy memory-mapped file reader with kernel advice management.
#[derive(uniffi::Object)]
pub struct UniFFIMmapReader {
    mapping: Option<SafeMmapAllocation>,
    file_size: u64,
    page_size: u32,
    path: String,
}

#[uniffi::export]
impl UniFFIMmapReader {
    /// Maps a local file into virtual memory with read-only protection.
    #[uniffi::constructor]
    pub fn open(path: String) -> Result<Arc<Self>, TTZipError> {
        let p = Path::new(&path);
        if !p.exists() {
            return Err(TTZipError::FileNotFound { path });
        }

        let file = File::open(p).map_err(|e| TTZipError::io_error(e, "failed to open file for mmap"))?;
        let meta = file.metadata().map_err(|e| TTZipError::io_error(e, "failed to read file metadata for mmap"))?;
        let file_size = meta.len();

        let page_size = {
            let sz = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
            if sz > 0 {
                sz as u32
            } else {
                4096
            }
        };

        if file_size == 0 {
            return Ok(Arc::new(Self {
                mapping: None,
                file_size: 0,
                page_size,
                path,
            }));
        }

        if file_size > usize::MAX as u64 {
            return Err(TTZipError::IoError {
                message: format!("File size {} exceeds system address space limits", file_size),
            });
        }

        let map_len = file_size as usize;
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                map_len,
                libc::PROT_READ,
                libc::MAP_PRIVATE,
                file.as_raw_fd(),
                0,
            )
        };

        if ptr == libc::MAP_FAILED {
            return Err(TTZipError::IoError {
                message: "libc mmap failed to map file into virtual address space".to_string(),
            });
        }

        // Apply initial sequential advice for high-throughput scanning
        unsafe {
            libc::madvise(ptr, map_len, libc::MADV_SEQUENTIAL);
        }

        Ok(Arc::new(Self {
            mapping: Some(SafeMmapAllocation {
                ptr: ptr as *const u8,
                len: map_len,
            }),
            file_size,
            page_size,
            path,
        }))
    }

    /// Issues kernel memory access advice (madvise) on the specified mapped byte range.
    pub fn advise(&self, advice: UniFFIMmapAdvice, offset: u64, length: u64) -> Result<(), TTZipError> {
        let libc_advice = match advice {
            UniFFIMmapAdvice::Normal => libc::MADV_NORMAL,
            UniFFIMmapAdvice::Sequential => libc::MADV_SEQUENTIAL,
            UniFFIMmapAdvice::Random => libc::MADV_RANDOM,
            UniFFIMmapAdvice::WillNeed => libc::MADV_WILLNEED,
            UniFFIMmapAdvice::DontNeed => libc::MADV_DONTNEED,
        };

        if let Some(ref m) = self.mapping {
            if self.file_size == 0 || m.len == 0 {
                return Ok(());
            }
            if offset >= self.file_size {
                return Err(TTZipError::IoError {
                    message: format!("Offset {} exceeds mapped file size {}", offset, self.file_size),
                });
            }

            let page_size_u64 = self.page_size as u64;
            let page_mask = page_size_u64.saturating_sub(1);
            let aligned_offset = offset & !page_mask;
            let page_delta = (offset - aligned_offset) as usize;

            let eff_len = if length == 0 {
                (self.file_size - offset) as usize
            } else {
                (length as usize).min((self.file_size - offset) as usize)
            };

            let aligned_len = (eff_len + page_delta).min(m.len.saturating_sub(aligned_offset as usize));

            if aligned_len > 0 {
                let target_ptr = unsafe { m.ptr.add(aligned_offset as usize) as *mut libc::c_void };
                let res = unsafe { libc::madvise(target_ptr, aligned_len, libc_advice) };
                if res != 0 {
                    return Err(TTZipError::IoError {
                        message: format!("madvise failed with status: {}", res),
                    });
                }
            }
        }
        Ok(())
    }

    /// Queries aggregated mapping statistics and memory bounds.
    pub fn stats(&self) -> UniFFIMmapStats {
        UniFFIMmapStats {
            file_size: self.file_size,
            mapped_size: self.mapping.as_ref().map(|m| m.len as u64).unwrap_or(0),
            is_empty: self.file_size == 0,
            page_size: self.page_size,
            is_readonly: true,
        }
    }

    /// Returns the mapped file path.
    pub fn path(&self) -> String {
        self.path.clone()
    }

    /// Returns total mapped file size in bytes.
    pub fn len(&self) -> u64 {
        self.file_size
    }

    /// Returns `true` if the mapped file is 0 bytes.
    pub fn is_empty(&self) -> bool {
        self.file_size == 0
    }

    /// Reads a bounded slice descriptor starting from offset with requested length.
    pub fn read_slice(&self, offset: u64, length: u64) -> Result<UniFFIMmapSlice, TTZipError> {
        if offset > self.file_size {
            return Err(TTZipError::IoError {
                message: format!("Offset {} out of bounds for size {}", offset, self.file_size),
            });
        }

        if self.mapping.is_none() || self.file_size == 0 || length == 0 {
            return Ok(UniFFIMmapSlice {
                offset,
                length: 0,
                data: Vec::new(),
            });
        }

        let m = self.mapping.as_ref().unwrap();
        let avail = (self.file_size - offset) as usize;
        let to_read = (length as usize).min(avail);
        let slice = unsafe { std::slice::from_raw_parts(m.ptr.add(offset as usize), to_read) };

        Ok(UniFFIMmapSlice {
            offset,
            length: to_read as u64,
            data: slice.to_vec(),
        })
    }

    /// Reads raw bytes within the specified range.
    pub fn read_bytes(&self, offset: u64, length: u64) -> Result<Vec<u8>, TTZipError> {
        Ok(self.read_slice(offset, length)?.data)
    }

    /// Reads all mapped file bytes into memory.
    pub fn read_all(&self) -> Result<Vec<u8>, TTZipError> {
        self.read_bytes(0, self.file_size)
    }

    /// Partitions the mapped file into fixed-size chunk slices.
    pub fn read_chunks(&self, chunk_size: u64) -> Result<Vec<UniFFIMmapSlice>, TTZipError> {
        if chunk_size == 0 {
            return Err(TTZipError::IoError {
                message: "Chunk size must be greater than zero".to_string(),
            });
        }

        if self.is_empty() {
            return Ok(Vec::new());
        }

        let mut slices = Vec::new();
        let mut curr_offset: u64 = 0;

        while curr_offset < self.file_size {
            let slice = self.read_slice(curr_offset, chunk_size)?;
            if slice.length == 0 {
                break;
            }
            curr_offset += slice.length;
            slices.push(slice);
        }

        Ok(slices)
    }

    /// Fast subsequence pattern matching over mapped memory.
    pub fn search_subsequence(&self, pattern: Vec<u8>, start_offset: u64) -> Option<u64> {
        if pattern.is_empty() {
            return Some(start_offset.min(self.file_size));
        }

        if start_offset >= self.file_size || self.mapping.is_none() {
            return None;
        }

        let m = self.mapping.as_ref().unwrap();
        let remaining_len = (self.file_size - start_offset) as usize;
        if pattern.len() > remaining_len {
            return None;
        }

        let slice = unsafe { std::slice::from_raw_parts(m.ptr.add(start_offset as usize), remaining_len) };
        slice
            .windows(pattern.len())
            .position(|window| window == pattern.as_slice())
            .map(|pos| start_offset + pos as u64)
    }

    /// Computes CRC32 checksum over the requested mapped range.
    pub fn compute_crc32(&self, offset: u64, length: u64) -> Result<u32, TTZipError> {
        if offset > self.file_size {
            return Err(TTZipError::IoError {
                message: format!("Offset {} out of bounds for size {}", offset, self.file_size),
            });
        }

        if self.mapping.is_none() || self.file_size == 0 || length == 0 {
            return Ok(0);
        }

        let m = self.mapping.as_ref().unwrap();
        let avail = (self.file_size - offset) as usize;
        let to_read = (length as usize).min(avail);
        let slice = unsafe { std::slice::from_raw_parts(m.ptr.add(offset as usize), to_read) };

        let mut hasher = crc32fast::Hasher::new();
        hasher.update(slice);
        Ok(hasher.finalize())
    }

    /// Computes hardware-accelerated XXH3-64 checksum over the requested mapped range.
    pub fn compute_xxh3(&self, offset: u64, length: u64) -> Result<u64, TTZipError> {
        if offset > self.file_size {
            return Err(TTZipError::IoError {
                message: format!("Offset {} out of bounds for size {}", offset, self.file_size),
            });
        }

        if self.mapping.is_none() || self.file_size == 0 || length == 0 {
            return Ok(crate::crypto::xxh3::xxh3_64(b""));
        }

        let m = self.mapping.as_ref().unwrap();
        let avail = (self.file_size - offset) as usize;
        let to_read = (length as usize).min(avail);
        let slice = unsafe { std::slice::from_raw_parts(m.ptr.add(offset as usize), to_read) };

        Ok(crate::crypto::xxh3::xxh3_64(slice))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_mmap_reader_non_existent_file() {
        let res = UniFFIMmapReader::open("/tmp/non_existent_mmap_file_ttzip.bin".to_string());
        assert!(res.is_err());
    }

    #[test]
    fn test_mmap_reader_empty_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let empty_path = temp_dir.path().join("empty.bin");
        File::create(&empty_path).unwrap();

        let reader = UniFFIMmapReader::open(empty_path.to_str().unwrap().to_string()).unwrap();
        assert_eq!(reader.len(), 0);
        assert!(reader.is_empty());

        let stats = reader.stats();
        assert!(stats.is_empty);
        assert_eq!(stats.file_size, 0);

        let data = reader.read_all().unwrap();
        assert!(data.is_empty());

        let chunks = reader.read_chunks(1024).unwrap();
        assert!(chunks.is_empty());

        assert_eq!(reader.compute_crc32(0, 0).unwrap(), 0);
    }

    #[test]
    fn test_mmap_reader_payload_and_slicing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("payload.bin");
        let payload = b"Hello, TTZip Zero-Copy Memory Mapped Stream Engine!";
        {
            let mut f = File::create(&file_path).unwrap();
            f.write_all(payload).unwrap();
        }

        let reader = UniFFIMmapReader::open(file_path.to_str().unwrap().to_string()).unwrap();
        assert_eq!(reader.len(), payload.len() as u64);
        assert!(!reader.is_empty());

        // Read all
        let read_back = reader.read_all().unwrap();
        assert_eq!(&read_back, payload);

        // Read slice
        let slice = reader.read_slice(7, 5).unwrap();
        assert_eq!(slice.offset, 7);
        assert_eq!(slice.length, 5);
        assert_eq!(&slice.data, b"TTZip");

        // Subsequence search
        let found = reader.search_subsequence(b"Zero-Copy".to_vec(), 0);
        assert_eq!(found, Some(13));

        let not_found = reader.search_subsequence(b"NonExistent".to_vec(), 0);
        assert_eq!(not_found, None);

        // Advise kernel
        assert!(reader.advise(UniFFIMmapAdvice::Sequential, 0, reader.len()).is_ok());
        assert!(reader.advise(UniFFIMmapAdvice::WillNeed, 0, 16).is_ok());
        assert!(reader.advise(UniFFIMmapAdvice::DontNeed, 0, reader.len()).is_ok());

        // Chunking
        let chunks = reader.read_chunks(10).unwrap();
        assert!(!chunks.is_empty());
        let combined: Vec<u8> = chunks.into_iter().flat_map(|c| c.data).collect();
        assert_eq!(&combined, payload);

        // Checksums
        let crc = reader.compute_crc32(0, reader.len()).unwrap();
        assert_ne!(crc, 0);

        let xxh = reader.compute_xxh3(0, reader.len()).unwrap();
        assert_ne!(xxh, 0);
    }
}
