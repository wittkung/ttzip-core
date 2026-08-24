// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

//! Pure, idiomatic Safe Rust SDK 2.0 builders and readers for archive creation,
//! extraction, and inspection without raw C pointers.

use crate::archive::unified::create::create_archive;
use crate::archive::unified::extract::extract_archive_with_metrics;
use crate::archive::unified::extract_single::extract_single_entry_memory;
use crate::archive::unified::inspect::inspect_archive;
use crate::types::{
    TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions,
    TTZipEncryptionMethod, TTZipEntryMetadata, TTZipExtractOptions, TTZipStatus,
};
use std::ffi::{CStr, CString};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Pure Rust metadata descriptor for an entry inside an archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveEntryInfo {
    pub path: String,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub mtime_epoch_secs: i64,
    pub mode: u32,
    pub is_directory: bool,
    pub is_encrypted: bool,
    pub compression_method: u16,
    pub detected_encoding: Option<String>,
}

impl ArchiveEntryInfo {
    /// Constructs an `ArchiveEntryInfo` from a C-ABI metadata struct safely.
    pub fn from_raw_metadata(meta: &TTZipEntryMetadata) -> Self {
        let path = if meta.path.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(meta.path) }
                .to_string_lossy()
                .into_owned()
        };

        let detected_encoding = if meta.detected_encoding.is_null() {
            None
        } else {
            Some(
                unsafe { CStr::from_ptr(meta.detected_encoding) }
                    .to_string_lossy()
                    .into_owned(),
            )
        };

        Self {
            path,
            uncompressed_size: meta.uncompressed_size,
            compressed_size: meta.compressed_size,
            crc32: meta.crc32,
            mtime_epoch_secs: meta.mtime_epoch_secs,
            mode: meta.mode,
            is_directory: meta.is_directory,
            is_encrypted: meta.is_encrypted,
            compression_method: meta.compression_method,
            detected_encoding,
        }
    }
}

pub type ProgressClosure = Box<dyn FnMut(u64, u64, &str) -> bool + Send>;

/// Pure, safe Rust builder for constructing archives.
pub struct ArchiveBuilder {
    source_paths: Vec<PathBuf>,
    destination_path: Option<PathBuf>,
    format: TTZipArchiveFormat,
    level: TTZipCompressionLevel,
    encryption: TTZipEncryptionMethod,
    password: Option<String>,
    thread_budget: u32,
    solid_block_size_mb: u32,
    split_volume_size_bytes: u64,
    progress_callback: Option<ProgressClosure>,
}

impl Default for ArchiveBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ArchiveBuilder {
    /// Creates a new, default `ArchiveBuilder`.
    pub fn new() -> Self {
        Self {
            source_paths: Vec::new(),
            destination_path: None,
            format: TTZipArchiveFormat::Zip,
            level: TTZipCompressionLevel::Normal,
            encryption: TTZipEncryptionMethod::None,
            password: None,
            thread_budget: 0,
            solid_block_size_mb: 64,
            split_volume_size_bytes: 0,
            progress_callback: None,
        }
    }

    /// Adds a source file or directory path to include in the archive.
    pub fn add_source(mut self, path: impl AsRef<Path>) -> Self {
        self.source_paths.push(path.as_ref().to_path_buf());
        self
    }

    /// Adds multiple source file or directory paths.
    pub fn add_sources(mut self, paths: impl IntoIterator<Item = impl AsRef<Path>>) -> Self {
        for p in paths {
            self.source_paths.push(p.as_ref().to_path_buf());
        }
        self
    }

    /// Sets the destination archive output path.
    pub fn destination(mut self, path: impl AsRef<Path>) -> Self {
        self.destination_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Sets the archive format.
    pub fn format(mut self, format: TTZipArchiveFormat) -> Self {
        self.format = format;
        self
    }

    /// Sets the compression level.
    pub fn level(mut self, level: TTZipCompressionLevel) -> Self {
        self.level = level;
        self
    }

    /// Sets the encryption method.
    pub fn encryption(mut self, encryption: TTZipEncryptionMethod) -> Self {
        self.encryption = encryption;
        self
    }

    /// Sets the archive password for encryption.
    pub fn password(mut self, password: impl Into<String>) -> Self {
        let pwd = password.into();
        if self.encryption == TTZipEncryptionMethod::None {
            self.encryption = TTZipEncryptionMethod::Aes256;
        }
        self.password = Some(pwd);
        self
    }

    /// Sets the worker thread budget (0 for auto / hardware concurrency).
    pub fn thread_budget(mut self, threads: u32) -> Self {
        self.thread_budget = threads;
        self
    }

    /// Sets the solid block size in megabytes for 7z archives.
    pub fn solid_block_size_mb(mut self, mb: u32) -> Self {
        self.solid_block_size_mb = mb;
        self
    }

    /// Sets the split volume size in bytes for multi-volume archives (0 to disable).
    pub fn split_volume_size_bytes(mut self, bytes: u64) -> Self {
        self.split_volume_size_bytes = bytes;
        self
    }

    /// Registers a progress callback closure `|processed_bytes, total_bytes, current_entry| -> should_continue`.
    pub fn on_progress<F>(mut self, callback: F) -> Self
    where
        F: FnMut(u64, u64, &str) -> bool + Send + 'static,
    {
        self.progress_callback = Some(Box::new(callback));
        self
    }

    /// Builds the archive writing to `destination_path`.
    pub fn build_to_file(&mut self, destination_path: impl AsRef<Path>) -> Result<(), TTZipStatus> {
        if self.source_paths.is_empty() {
            return Err(TTZipStatus::ErrInvalidParam);
        }

        let pwd_cstr = self
            .password
            .as_ref()
            .and_then(|p| CString::new(p.as_str()).ok());

        let mut cb_ptr = self.progress_callback.take();

        unsafe extern "C" fn progress_shim(
            processed: u64,
            total: u64,
            current_entry: *const libc::c_char,
            user_data: *mut libc::c_void,
        ) -> bool {
            if user_data.is_null() {
                return true;
            }
            let cb = &mut *(user_data as *mut ProgressClosure);
            let entry_name = if current_entry.is_null() {
                ""
            } else {
                CStr::from_ptr(current_entry).to_str().unwrap_or("")
            };
            cb(processed, total, entry_name)
        }

        let has_cb = cb_ptr.is_some();
        let user_data = match cb_ptr.as_mut() {
            Some(boxed) => boxed as *mut ProgressClosure as *mut libc::c_void,
            None => std::ptr::null_mut(),
        };

        let options = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_2,
            format: self.format,
            level: self.level,
            encryption: self.encryption,
            password: pwd_cstr.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null()),
            thread_budget: self.thread_budget,
            solid_block_size_mb: self.solid_block_size_mb,
            progress_callback: if has_cb { Some(progress_shim) } else { None },
            user_data,
        };

        let result = create_archive(
            &self.source_paths,
            destination_path.as_ref(),
            &options,
            self.split_volume_size_bytes,
        );

        self.progress_callback = cb_ptr;
        result
    }

    /// Builds the archive using the configured destination path.
    pub fn build(&mut self) -> Result<(), TTZipStatus> {
        let dest = self.destination_path.clone().ok_or(TTZipStatus::ErrInvalidParam)?;
        self.build_to_file(dest)
    }

    /// Builds the archive streaming into a standard `std::io::Write` target.
    pub fn build_to_writer<W: Write>(&mut self, mut writer: W) -> Result<u64, TTZipStatus> {
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join(format!("ttzip_build_{}.tmp", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        self.build_to_file(&temp_file)?;

        let mut reader = std::fs::File::open(&temp_file).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let copied = std::io::copy(&mut reader, &mut writer).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        drop(reader);
        let _ = std::fs::remove_file(temp_file);
        Ok(copied)
    }
}

/// Pure, safe Rust builder for extracting archives.
pub struct ExtractBuilder {
    archive_path: Option<PathBuf>,
    destination_path: Option<PathBuf>,
    password: Option<String>,
    thread_budget: u32,
    overwrite_existing: bool,
    preserve_permissions: bool,
    dry_run: bool,
    progress_callback: Option<ProgressClosure>,
}

impl Default for ExtractBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtractBuilder {
    /// Creates a new, default `ExtractBuilder`.
    pub fn new() -> Self {
        Self {
            archive_path: None,
            destination_path: None,
            password: None,
            thread_budget: 0,
            overwrite_existing: true,
            preserve_permissions: true,
            dry_run: false,
            progress_callback: None,
        }
    }

    /// Sets the source archive path to extract.
    pub fn source(mut self, path: impl AsRef<Path>) -> Self {
        self.archive_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Sets the destination directory for extracted files.
    pub fn destination(mut self, path: impl AsRef<Path>) -> Self {
        self.destination_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Sets the decryption password for encrypted archives.
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    /// Sets the thread budget for parallel extraction.
    pub fn thread_budget(mut self, threads: u32) -> Self {
        self.thread_budget = threads;
        self
    }

    /// Configures whether existing files should be overwritten.
    pub fn overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite_existing = overwrite;
        self
    }

    /// Configures whether filesystem permissions and timestamps should be preserved.
    pub fn preserve_permissions(mut self, preserve: bool) -> Self {
        self.preserve_permissions = preserve;
        self
    }

    /// Enables or disables dry-run mode (verifies headers without writing files).
    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Registers a progress callback closure `|processed_bytes, total_bytes, current_entry| -> should_continue`.
    pub fn on_progress<F>(mut self, callback: F) -> Self
    where
        F: FnMut(u64, u64, &str) -> bool + Send + 'static,
    {
        self.progress_callback = Some(Box::new(callback));
        self
    }

    /// Performs extraction to `destination_path` and returns total uncompressed bytes extracted.
    pub fn extract_to(&mut self, destination_path: impl AsRef<Path>) -> Result<u64, TTZipStatus> {
        let arch = self.archive_path.as_ref().ok_or(TTZipStatus::ErrInvalidParam)?;
        let pwd_cstr = self
            .password
            .as_ref()
            .and_then(|p| CString::new(p.as_str()).ok());

        let mut cb_ptr = self.progress_callback.take();

        unsafe extern "C" fn progress_shim(
            processed: u64,
            total: u64,
            current_entry: *const libc::c_char,
            user_data: *mut libc::c_void,
        ) -> bool {
            if user_data.is_null() {
                return true;
            }
            let cb = &mut *(user_data as *mut ProgressClosure);
            let entry_name = if current_entry.is_null() {
                ""
            } else {
                CStr::from_ptr(current_entry).to_str().unwrap_or("")
            };
            cb(processed, total, entry_name)
        }

        let has_cb = cb_ptr.is_some();
        let user_data = match cb_ptr.as_mut() {
            Some(boxed) => boxed as *mut ProgressClosure as *mut libc::c_void,
            None => std::ptr::null_mut(),
        };

        let dest_c = CString::new(destination_path.as_ref().to_str().unwrap_or("")).ok();

        let options = TTZipExtractOptions {
            struct_size: std::mem::size_of::<TTZipExtractOptions>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_2,
            destination_path: dest_c.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null()),
            password: pwd_cstr.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null()),
            thread_budget: self.thread_budget,
            overwrite_existing: self.overwrite_existing,
            preserve_permissions: self.preserve_permissions,
            dry_run: self.dry_run,
            progress_callback: if has_cb { Some(progress_shim) } else { None },
            user_data,
        };

        let result = extract_archive_with_metrics(arch, destination_path.as_ref(), &options);
        self.progress_callback = cb_ptr;
        result
    }

    /// Performs extraction using the configured destination path.
    pub fn extract(&mut self) -> Result<u64, TTZipStatus> {
        let dest = self.destination_path.clone().ok_or(TTZipStatus::ErrInvalidParam)?;
        self.extract_to(dest)
    }

    /// Extracts a single entry directly into an in-memory buffer with zero disk I/O.
    pub fn extract_single_to_memory(&self, entry_path: &str) -> Result<Vec<u8>, TTZipStatus> {
        let arch = self.archive_path.as_ref().ok_or(TTZipStatus::ErrInvalidParam)?;
        extract_single_entry_memory(arch, Some(entry_path), -1, self.password.as_deref())
    }
}

/// Pure, safe Rust reader for inspecting and querying archives.
pub struct ArchiveReader {
    archive_path: PathBuf,
    password: Option<String>,
}

impl ArchiveReader {
    /// Opens an archive file for inspection.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, TTZipStatus> {
        let p = path.as_ref().to_path_buf();
        if !p.exists() {
            return Err(TTZipStatus::ErrFileNotFound);
        }
        Ok(Self {
            archive_path: p,
            password: None,
        })
    }

    /// Sets the archive password for inspecting encrypted headers.
    pub fn with_password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    /// Returns the path to the archive file.
    pub fn path(&self) -> &Path {
        &self.archive_path
    }

    /// Returns the list of all entries and their metadata in the archive.
    pub fn entries(&self) -> Result<Vec<ArchiveEntryInfo>, TTZipStatus> {
        let mut entries = Vec::new();

        unsafe extern "C" fn inspect_callback(
            entry: *const TTZipEntryMetadata,
            user_data: *mut libc::c_void,
        ) -> bool {
            if entry.is_null() || user_data.is_null() {
                return false;
            }
            let list = &mut *(user_data as *mut Vec<ArchiveEntryInfo>);
            let info = ArchiveEntryInfo::from_raw_metadata(&*entry);
            list.push(info);
            true
        }

        inspect_archive(
            &self.archive_path,
            self.password.as_deref(),
            true,
            Some(inspect_callback),
            &mut entries as *mut Vec<ArchiveEntryInfo> as *mut libc::c_void,
        )?;

        Ok(entries)
    }

    /// Extracts a single entry directly to memory.
    pub fn extract_entry(&self, entry_path: &str) -> Result<Vec<u8>, TTZipStatus> {
        extract_single_entry_memory(
            &self.archive_path,
            Some(entry_path),
            -1,
            self.password.as_deref(),
        )
    }

    /// Extracts all entries to `destination_path`.
    pub fn extract_all(&self, destination_path: impl AsRef<Path>) -> Result<u64, TTZipStatus> {
        let mut builder = ExtractBuilder::new().source(&self.archive_path);
        if let Some(ref pwd) = self.password {
            builder = builder.password(pwd);
        }
        builder.extract_to(destination_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_pure_rust_archive_builder_reader_extract_flow() {
        let dir = tempdir().unwrap();
        let src_dir = dir.path().join("pure_rust_src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let f1 = src_dir.join("file1.txt");
        let f2 = src_dir.join("file2.bin");
        std::fs::write(&f1, b"Pure Rust ArchiveBuilder Text Payload").unwrap();
        std::fs::write(&f2, vec![0x99u8; 1024]).unwrap();

        let archive_out = dir.path().join("output.zip");

        // 1. Build Archive using ArchiveBuilder
        let mut progress_count = 0;
        let mut builder = ArchiveBuilder::new()
            .add_source(&src_dir)
            .destination(&archive_out)
            .format(TTZipArchiveFormat::Zip)
            .level(TTZipCompressionLevel::Normal)
            .thread_budget(2)
            .on_progress(move |_processed, _total, _entry| {
                progress_count += 1;
                true
            });

        builder.build().expect("ArchiveBuilder build must succeed");
        assert!(archive_out.exists());

        // 2. Read Archive using ArchiveReader
        let reader = ArchiveReader::open(&archive_out).expect("ArchiveReader open must succeed");
        let entries = reader.entries().expect("ArchiveReader entries must succeed");
        assert!(entries.len() >= 2);

        let entry_names: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
        assert!(entry_names.iter().any(|n| n.contains("file1.txt")));
        assert!(entry_names.iter().any(|n| n.contains("file2.bin")));

        // 3. Extract Single Entry to Memory
        let mem_bytes = reader
            .extract_entry("pure_rust_src/file1.txt")
            .expect("Extract entry to memory must succeed");
        assert_eq!(mem_bytes, b"Pure Rust ArchiveBuilder Text Payload");

        // 4. Extract All to Destination using ExtractBuilder
        let dest_dir = dir.path().join("pure_rust_extracted");
        let mut ext_builder = ExtractBuilder::new()
            .source(&archive_out)
            .destination(&dest_dir)
            .thread_budget(2)
            .overwrite(true);

        let extracted_bytes = ext_builder.extract().expect("ExtractBuilder extract must succeed");
        assert!(extracted_bytes > 0);
        assert!(dest_dir.join("pure_rust_src/file1.txt").exists());
        assert!(dest_dir.join("pure_rust_src/file2.bin").exists());

        assert_eq!(
            std::fs::read(dest_dir.join("pure_rust_src/file1.txt")).unwrap(),
            b"Pure Rust ArchiveBuilder Text Payload"
        );
    }
}

