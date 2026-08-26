// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Operation configuration options, callbacks, and memory management kinds.

use libc::{c_char, c_void};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use super::dto::TTZipEntryMetadata;
use super::formats::{TTZipArchiveFormat, TTZipCompressionLevel, TTZipEncryptionMethod};

pub const TTZIP_ABI_VERSION_2: u32 = 2;
pub const TTZIP_ABI_VERSION: u32 = TTZIP_ABI_VERSION_2;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TTZipLogLevel {
    Debug = 0,
    Info = 1,
    Warning = 2,
    Error = 3,
}

/// Memory kind enumeration for canonical universal deallocator `ttzip_free`.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TTZipMemoryKind {
    /// C-string allocated by Rust (e.g. CString::into_raw).
    String = 0,
    /// Raw byte buffer or buffer descriptor.
    Buffer = 1,
    /// Aligned buffer allocated via platform alloc / posix_memalign.
    Aligned = 2,
    /// Thread-safe error descriptor allocated via Box<TTZipError>.
    Error = 3,
    /// VFS tree handle.
    VfsTree = 4,
    /// VFS cache pool handle.
    VfsCache = 5,
    /// Filter DSL engine handle.
    Filter = 6,
    /// Path filter handle.
    PathFilter = 7,
    /// Split volume reader handle.
    SplitReader = 8,
    /// Split volume writer handle.
    SplitWriter = 9,
    /// Stream reader handle.
    StreamReader = 10,
    /// Stream writer handle.
    StreamWriter = 11,
    /// Cancellation token handle.
    CancellationToken = 12,
    /// In-place archive mutation session handle.
    InPlaceSession = 13,
}

pub type TTZipProgressCallback = Option<
    unsafe extern "C" fn(
        processed_bytes: u64,
        total_bytes: u64,
        current_entry: *const c_char,
        user_data: *mut c_void,
    ) -> bool,
>;

pub type TTZipInspectCallback = Option<
    unsafe extern "C" fn(
        entry: *const TTZipEntryMetadata,
        user_data: *mut c_void,
    ) -> bool,
>;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TTZipExtractOptions {
    pub struct_size: u32,
    pub abi_version: u32,
    pub destination_path: *const c_char,
    pub password: *const c_char,
    pub thread_budget: u32,
    pub overwrite_existing: bool,
    pub preserve_permissions: bool,
    pub dry_run: bool,
    pub progress_callback: TTZipProgressCallback,
    pub user_data: *mut c_void,
}

impl Default for TTZipExtractOptions {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            abi_version: TTZIP_ABI_VERSION_2,
            destination_path: std::ptr::null(),
            password: std::ptr::null(),
            thread_budget: 0,
            overwrite_existing: true,
            preserve_permissions: true,
            dry_run: false,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TTZipCreateOptions {
    pub struct_size: u32,
    pub abi_version: u32,
    pub format: TTZipArchiveFormat,
    pub level: TTZipCompressionLevel,
    pub encryption: TTZipEncryptionMethod,
    pub password: *const c_char,
    pub thread_budget: u32,
    pub solid_block_size_mb: u32,
    pub progress_callback: TTZipProgressCallback,
    pub user_data: *mut c_void,
}

impl Default for TTZipCreateOptions {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            abi_version: TTZIP_ABI_VERSION_2,
            format: TTZipArchiveFormat::Zip,
            level: TTZipCompressionLevel::Normal,
            encryption: TTZipEncryptionMethod::None,
            password: std::ptr::null(),
            thread_budget: 0,
            solid_block_size_mb: 64,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Zeroize)]
#[zeroize(drop)]
pub struct TTZipAes256Context {
    pub key: [u8; 32],
    pub iv_or_counter: [u8; 16],
    pub round_keys_enc: [u8; 240], // 15 rounds * 16 bytes
    pub round_keys_dec: [u8; 240],
}
