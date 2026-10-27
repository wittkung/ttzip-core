// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Universal Virtual Filesystem (VFS) abstraction traits and canonical providers.

use std::fs::File;
use std::io::{self, Error, ErrorKind};
use std::path::{Path, PathBuf};

/// Universal Virtual Filesystem provider for random access archive extraction,
/// high-throughput positioned reads, Copy-on-Write (CoW) cloning, and sparse file allocation.
pub trait ArchiveVfsProvider: Send + Sync {
    /// Reads exact bytes at the specified absolute offset without mutating seek position.
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()>;

    /// Returns the total logical size in bytes of the underlying archive or container.
    fn size(&self) -> io::Result<u64>;

    /// Attempts to perform a Copy-on-Write (CoW) clone of a byte range into the destination path.
    ///
    /// Returns `Ok(true)` if hardware/filesystem CoW was successfully executed.
    /// Returns `Ok(false)` if the underlying filesystem does not support CoW or if cross-device boundaries
    /// prevent cloning, indicating the caller should fallback to standard stream copy.
    fn try_cow_clone_range(&self, src_offset: u64, dst_path: &Path, len: u64) -> io::Result<bool>;

    /// Pre-allocates a sparse file at `path` of length `len` bytes without allocating physical disk blocks.
    fn allocate_sparse(&self, path: &Path, len: u64) -> io::Result<()>;
}

/// Canonical local filesystem provider backed by an open [`File`].
pub struct LocalArchiveVfsProvider {
    file: File,
    path: Option<PathBuf>,
}

impl LocalArchiveVfsProvider {
    /// Opens a local file from disk at `path`.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let p = path.as_ref().to_path_buf();
        let file = File::open(&p)?;
        let provider = Self {
            file,
            path: Some(p),
        };
        provider.enable_sequential_readahead();
        Ok(provider)
    }

    /// Advises the operating system kernel to perform aggressive sequential readahead for the file.
    ///
    /// - On macOS: issues `fcntl(fd, F_RDAHEAD, 1)`.
    /// - On Linux: issues `posix_fadvise(fd, 0, 0, POSIX_FADV_SEQUENTIAL)`.
    pub fn enable_sequential_readahead(&self) {
        #[cfg(target_vendor = "apple")]
        {
            use std::os::unix::io::AsRawFd;
            unsafe {
                libc::fcntl(self.file.as_raw_fd(), libc::F_RDAHEAD, 1);
            }
        }
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::io::AsRawFd;
            unsafe {
                libc::posix_fadvise(self.file.as_raw_fd(), 0, 0, libc::POSIX_FADV_SEQUENTIAL);
            }
        }
    }

    /// Creates a provider wrapping an existing open [`File`].
    pub fn from_file(file: File) -> Self {
        let provider = Self { file, path: None };
        provider.enable_sequential_readahead();
        provider
    }

    /// Creates a provider wrapping an existing open [`File`] and its known file path.
    pub fn from_file_with_path(file: File, path: impl Into<PathBuf>) -> Self {
        let provider = Self {
            file,
            path: Some(path.into()),
        };
        provider.enable_sequential_readahead();
        provider
    }

    /// Borrow the underlying [`File`].
    #[inline]
    pub fn file(&self) -> &File {
        &self.file
    }


    /// Borrow the optional filesystem path.
    #[inline]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Performs a stream fallback copy of `len` bytes starting at `src_offset` into `dst_path`.
    pub fn copy_range_fallback(&self, src_offset: u64, dst_path: &Path, len: u64) -> io::Result<u64> {
        crate::vfs::cow::copy_range_to_file(&self.file, src_offset, dst_path, len)
    }

    /// High-level extraction helper: attempts zero-cost CoW clone first, falling back to streaming copy.
    pub fn extract_range_to_file(&self, src_offset: u64, dst_path: &Path, len: u64) -> io::Result<u64> {
        if self.try_cow_clone_range(src_offset, dst_path, len)? {
            return Ok(len);
        }
        self.copy_range_fallback(src_offset, dst_path, len)
    }
}

impl ArchiveVfsProvider for LocalArchiveVfsProvider {
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileExt;
            self.file.read_exact_at(buf, offset)
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::FileExt;
            let mut total_read = 0;
            while total_read < buf.len() {
                let read = self.file.seek_read(&mut buf[total_read..], offset + total_read as u64)?;
                if read == 0 {
                    return Err(Error::new(
                        ErrorKind::UnexpectedEof,
                        "failed to fill whole buffer in read_exact_at",
                    ));
                }
                total_read += read;
            }
            Ok(())
        }

        #[cfg(not(any(unix, windows)))]
        {
            use std::io::{Read, Seek, SeekFrom};
            let mut clone = self.file.try_clone()?;
            clone.seek(SeekFrom::Start(offset))?;
            clone.read_exact(buf)
        }
    }

    fn size(&self) -> io::Result<u64> {
        self.file.metadata().map(|m| m.len())
    }

    fn try_cow_clone_range(&self, src_offset: u64, dst_path: &Path, len: u64) -> io::Result<bool> {
        crate::vfs::cow::try_cow_clone_file_range(
            &self.file,
            self.path.as_deref(),
            src_offset,
            dst_path,
            len,
        )
    }

    fn allocate_sparse(&self, path: &Path, len: u64) -> io::Result<()> {
        crate::vfs::sparse::allocate_sparse_file(path, len).map(|_| ())
    }
}

/// In-memory VFS provider backed by a continuous byte slice.
pub struct MemoryArchiveVfsProvider {
    data: Vec<u8>,
}

impl MemoryArchiveVfsProvider {
    /// Creates an in-memory VFS provider from the provided byte vector.
    pub fn new(data: impl Into<Vec<u8>>) -> Self {
        Self { data: data.into() }
    }

    /// Borrow the underlying byte slice.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }
}

impl ArchiveVfsProvider for MemoryArchiveVfsProvider {
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        let start = offset as usize;
        let end = start.checked_add(buf.len()).ok_or_else(|| {
            Error::new(ErrorKind::InvalidInput, "Offset overflow in read_exact_at")
        })?;

        if end > self.data.len() {
            return Err(Error::new(
                ErrorKind::UnexpectedEof,
                "Attempted to read past the end of in-memory VFS buffer",
            ));
        }

        buf.copy_from_slice(&self.data[start..end]);
        Ok(())
    }

    fn size(&self) -> io::Result<u64> {
        Ok(self.data.len() as u64)
    }

    fn try_cow_clone_range(&self, _src_offset: u64, _dst_path: &Path, _len: u64) -> io::Result<bool> {
        // Memory buffers do not possess physical filesystem extents
        Ok(false)
    }

    fn allocate_sparse(&self, path: &Path, len: u64) -> io::Result<()> {
        crate::vfs::sparse::allocate_sparse_file(path, len).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_memory_vfs_provider() {
        let sample = b"0123456789ABCDEFHelloMetastudyLine";
        let provider = MemoryArchiveVfsProvider::new(sample.to_vec());
        assert_eq!(provider.size().unwrap(), sample.len() as u64);

        let mut buf = [0u8; 5];
        provider.read_exact_at(10, &mut buf).unwrap();
        assert_eq!(&buf, b"ABCDE");

        // Out of bounds error check
        let mut large_buf = vec![0u8; 100];
        assert!(provider.read_exact_at(10, &mut large_buf).is_err());
    }

    #[test]
    fn test_local_vfs_provider_lifecycle() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("archive_source.bin");

        let content = b"HighThroughputMetastudyLineIndustrialVfsEngineVerificationPayload";
        {
            let mut f = File::create(&file_path).unwrap();
            f.write_all(content).unwrap();
        }

        let provider = LocalArchiveVfsProvider::open(&file_path).unwrap();
        assert_eq!(provider.size().unwrap(), content.len() as u64);

        let mut read_buf = [0u8; 10];
        provider.read_exact_at(14, &mut read_buf).unwrap();
        assert_eq!(&read_buf, b"MetastudyL");

        // Test extraction fallback
        let dst_extracted = dir.path().join("extracted_subrange.bin");
        let extracted_len = provider
            .extract_range_to_file(14, &dst_extracted, 10)
            .unwrap();
        assert_eq!(extracted_len, 10);
        let extracted_data = std::fs::read(&dst_extracted).unwrap();
        assert_eq!(&extracted_data, b"MetastudyL");

        // Test sparse allocation
        let sparse_dst = dir.path().join("sparse_test.bin");
        provider.allocate_sparse(&sparse_dst, 65536).unwrap();
        assert_eq!(std::fs::metadata(&sparse_dst).unwrap().len(), 65536);
    }

    #[test]
    fn test_local_vfs_cow_full_clone_on_apfs() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("full_source.bin");
        let payload = vec![0x42u8; 32768];
        {
            let mut f = File::create(&file_path).unwrap();
            f.write_all(&payload).unwrap();
        }

        let provider = LocalArchiveVfsProvider::open(&file_path).unwrap();
        let cloned_dst = dir.path().join("cloned_full.bin");
        let cow_success = provider
            .try_cow_clone_range(0, &cloned_dst, payload.len() as u64)
            .unwrap();

        // On macOS APFS, CoW clone should succeed
        #[cfg(target_os = "macos")]
        assert!(cow_success);

        // Verify content integrity
        if cow_success {
            let cloned_data = std::fs::read(&cloned_dst).unwrap();
            assert_eq!(cloned_data, payload);
        }
    }
}
