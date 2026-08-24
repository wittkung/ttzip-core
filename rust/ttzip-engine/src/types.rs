// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

//! Core types, error codes, and shared data structures for TTZip Rust Glue.

use std::cell::RefCell;
use libc::{c_char, c_void};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TTZipStatus {
    Ok = 0,
    Eof = 1,
    Cancelled = 2,
    ErrInvalidParam = -1,
    ErrFileNotFound = -2,
    ErrMmapFailed = -3,
    ErrCorruptHeader = -4,
    ErrInvalidOffset = -5,
    ErrArchiveInitFailed = -6,
    ErrOpenFailed = -7,
    ErrPathTooLong = -8,
    ErrOutOfMemory = -9,
    ErrInvalidPassword = -10,
    ErrExtractionFailed = -11,
    ErrCompressionFailed = -12,
    ErrSecurityViolation = -30,
    ErrPanicCaught = -99,
}

impl TTZipStatus {
    #[inline]
    #[must_use]
    pub const fn to_i32(self) -> i32 {
        self as i32
    }

    /// Returns `true` if the status represents a successful operation (`Ok`).
    #[inline]
    #[must_use]
    pub const fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }

    /// Returns `true` if the status indicates an error condition.
    #[inline]
    #[must_use]
    pub const fn is_err(self) -> bool {
        (self as i32) < 0
    }

    /// Returns a static English string description of the status code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            TTZipStatus::Ok => "Operation completed successfully",
            TTZipStatus::Eof => "End of archive / stream reached",
            TTZipStatus::Cancelled => "Operation cancelled by user",
            TTZipStatus::ErrInvalidParam => "Invalid parameter or configuration",
            TTZipStatus::ErrFileNotFound => "Archive or source file not found",
            TTZipStatus::ErrMmapFailed => "Memory mapping failed",
            TTZipStatus::ErrCorruptHeader => "Corrupt archive header or signature mismatch",
            TTZipStatus::ErrInvalidOffset => "Invalid entry offset or seek error",
            TTZipStatus::ErrArchiveInitFailed => "Failed to initialize archive context",
            TTZipStatus::ErrOpenFailed => "Failed to open archive stream",
            TTZipStatus::ErrPathTooLong => "File path exceeds system MAXPATHLEN",
            TTZipStatus::ErrOutOfMemory => "Out of memory",
            TTZipStatus::ErrInvalidPassword => "Invalid or missing archive password",
            TTZipStatus::ErrExtractionFailed => "Extraction error",
            TTZipStatus::ErrCompressionFailed => "Compression error",
            TTZipStatus::ErrSecurityViolation => "Security boundary violation (e.g. path traversal)",
            TTZipStatus::ErrPanicCaught => "Internal panic caught at FFI boundary",
        }
    }
}

/// Resolves effective thread budget: expands 0 (auto) to available hardware parallelism.
#[inline]
pub fn resolve_thread_budget(budget: u32) -> usize {
    if budget == 0 {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .clamp(1, 64)
    } else {
        (budget as usize).clamp(1, 64)
    }
}

impl core::fmt::Display for TTZipStatus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::error::Error for TTZipStatus {}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TTZipArchiveFormat {
    Auto = 0,
    Zip = 1,
    SevenZip = 2,
    Tar = 3,
    TarGz = 4,
    TarBz2 = 5,
    TarXz = 6,
    TarZstd = 7,
    Dmg = 8,
    Lzfse = 9,
    Snappy = 10,
    Unknown = 99,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TTZipCompressionLevel {
    Store = 0,
    Fastest = 1,
    Fast = 3,
    Normal = 6,
    Maximum = 9,
    Ultra = 12,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TTZipEncryptionMethod {
    None = 0,
    ZipCrypto = 1,
    Aes128 = 2,
    Aes192 = 3,
    Aes256 = 4,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TTZipLogLevel {
    Debug = 0,
    Info = 1,
    Warning = 2,
    Error = 3,
}

pub const TTZIP_ABI_VERSION_2: u32 = 2;
pub const TTZIP_ABI_VERSION: u32 = TTZIP_ABI_VERSION_2;

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

/// Zero-copy read-only contiguous byte buffer slice descriptor.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TTZipBufferRef {
    pub data: *const u8,
    pub len: usize,
}

impl TTZipBufferRef {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            data: std::ptr::null(),
            len: 0,
        }
    }

    #[inline]
    pub const fn from_slice(slice: &[u8]) -> Self {
        Self {
            data: slice.as_ptr(),
            len: slice.len(),
        }
    }

    #[inline]
    pub unsafe fn as_slice<'a>(&self) -> &'a [u8] {
        if self.data.is_null() || self.len == 0 {
            &[]
        } else {
            std::slice::from_raw_parts(self.data, self.len)
        }
    }
}

/// Zero-copy mutable contiguous byte buffer slice descriptor.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TTZipBufferMut {
    pub data: *mut u8,
    pub len: usize,
    pub capacity: usize,
}

impl TTZipBufferMut {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            data: std::ptr::null_mut(),
            len: 0,
            capacity: 0,
        }
    }

    #[inline]
    pub fn from_vec(vec: &mut Vec<u8>) -> Self {
        Self {
            data: vec.as_mut_ptr(),
            len: vec.len(),
            capacity: vec.capacity(),
        }
    }

    #[inline]
    pub unsafe fn as_slice<'a>(&self) -> &'a [u8] {
        if self.data.is_null() || self.len == 0 {
            &[]
        } else {
            std::slice::from_raw_parts(self.data, self.len)
        }
    }

    #[inline]
    pub unsafe fn as_mut_slice<'a>(&mut self) -> &'a mut [u8] {
        if self.data.is_null() || self.len == 0 {
            &mut []
        } else {
            std::slice::from_raw_parts_mut(self.data, self.len)
        }
    }
}

/// Thread-safe explicit diagnostic error descriptor envelope.
///
/// Allocated on failure across C-ABI 2.0 boundaries; must be released
/// by the caller via `ttzip_free(error, TTZipMemoryKind::Error)`.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct TTZipError {
    pub struct_size: u32,
    pub abi_version: u32,
    pub status_code: i32,
    pub system_errno: i32,
    pub byte_offset: u64,
    pub entry_path: [c_char; 256],
    pub message: [c_char; 512],
}

impl TTZipError {
    pub fn new(status: TTZipStatus, msg: &str, entry: Option<&str>, offset: u64, system_errno: i32) -> Self {
        let mut err = Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            abi_version: TTZIP_ABI_VERSION_2,
            status_code: status as i32,
            system_errno,
            byte_offset: offset,
            entry_path: [0; 256],
            message: [0; 512],
        };
        let msg_bytes = msg.as_bytes();
        let copy_len = msg_bytes.len().min(511);
        for i in 0..copy_len {
            err.message[i] = msg_bytes[i] as c_char;
        }

        if let Some(e) = entry {
            let e_bytes = e.as_bytes();
            let e_len = e_bytes.len().min(255);
            for i in 0..e_len {
                err.entry_path[i] = e_bytes[i] as c_char;
            }
        }
        err
    }

    #[inline]
    pub fn allocate(status: TTZipStatus, msg: &str, entry: Option<&str>, offset: u64, system_errno: i32) -> *mut Self {
        Box::into_raw(Box::new(Self::new(status, msg, entry, offset, system_errno)))
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        self.struct_size as usize == std::mem::size_of::<Self>() && self.abi_version == TTZIP_ABI_VERSION_2
    }
}

/// Helper function to safely populate out_error pointer if non-null.
#[inline]
pub unsafe fn set_out_error(
    out_error: *mut *mut TTZipError,
    status: TTZipStatus,
    msg: &str,
    entry: Option<&str>,
    offset: u64,
    system_errno: i32,
) {
    if !out_error.is_null() {
        *out_error = TTZipError::allocate(status, msg, entry, offset, system_errno);
    }
    set_last_error(status, msg, entry, offset);
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TTZipEntryMetadata {
    pub struct_size: u32,
    pub abi_version: u32,
    pub path: *const c_char,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub mtime_epoch_secs: i64,
    pub mode: u32,
    pub is_directory: bool,
    pub is_encrypted: bool,
    pub compression_method: u16,
    pub detected_encoding: *const c_char,
}

impl Default for TTZipEntryMetadata {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            abi_version: TTZIP_ABI_VERSION_2,
            path: std::ptr::null(),
            uncompressed_size: 0,
            compressed_size: 0,
            crc32: 0,
            mtime_epoch_secs: 0,
            mode: 0,
            is_directory: false,
            is_encrypted: false,
            compression_method: 0,
            detected_encoding: std::ptr::null(),
        }
    }
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

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TTZipErrorInfo {
    pub struct_size: u32,
    pub abi_version: u32,
    pub status: TTZipStatus,
    pub error_code: i32,
    pub message: [c_char; 512],
    pub entry_path: [c_char; 256],
    pub offset: u64,
}

impl TTZipErrorInfo {
    pub const fn empty() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            abi_version: TTZIP_ABI_VERSION_2,
            status: TTZipStatus::Ok,
            error_code: 0,
            message: [0; 512],
            entry_path: [0; 256],
            offset: 0,
        }
    }

    pub fn populate(&mut self, status: TTZipStatus, msg: &str, entry: Option<&str>, offset: u64) {
        self.struct_size = std::mem::size_of::<Self>() as u32;
        self.abi_version = TTZIP_ABI_VERSION_2;
        self.status = status;
        self.error_code = status as i32;
        self.offset = offset;
        self.message.fill(0);
        let msg_bytes = msg.as_bytes();
        let copy_len = msg_bytes.len().min(511);
        for i in 0..copy_len {
            self.message[i] = msg_bytes[i] as c_char;
        }

        self.entry_path.fill(0);
        if let Some(e) = entry {
            let e_bytes = e.as_bytes();
            let e_len = e_bytes.len().min(255);
            for i in 0..e_len {
                self.entry_path[i] = e_bytes[i] as c_char;
            }
        }
    }
}

/// Helper function to safely populate out_error pointer if non-null
#[inline]
pub unsafe fn write_error_info(
    out_error: *mut TTZipErrorInfo,
    status: TTZipStatus,
    msg: &str,
    entry: Option<&str>,
    offset: u64,
) {
    if !out_error.is_null() {
        (*out_error).populate(status, msg, entry, offset);
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct DiagnosticErrorContext {
    pub status: TTZipStatus,
    pub message: [u8; 512],
    pub entry_path: [u8; 256],
    pub offset: u64,
}

impl DiagnosticErrorContext {
    pub const fn empty() -> Self {
        Self {
            status: TTZipStatus::Ok,
            message: [0u8; 512],
            entry_path: [0u8; 256],
            offset: 0,
        }
    }
}

thread_local! {
    static LAST_ERROR: RefCell<DiagnosticErrorContext> = const { RefCell::new(DiagnosticErrorContext::empty()) };
}

pub fn set_last_error(status: TTZipStatus, msg: &str, entry: Option<&str>, offset: u64) {
    LAST_ERROR.with(|cell| {
        let mut err = cell.borrow_mut();
        err.status = status;
        err.offset = offset;
        err.message.fill(0);
        let msg_bytes = msg.as_bytes();
        let copy_len = msg_bytes.len().min(511);
        err.message[..copy_len].copy_from_slice(&msg_bytes[..copy_len]);

        err.entry_path.fill(0);
        if let Some(e) = entry {
            let e_bytes = e.as_bytes();
            let e_len = e_bytes.len().min(255);
            err.entry_path[..e_len].copy_from_slice(&e_bytes[..e_len]);
        }
    });
}

/// Copies the thread-local error info into the caller-provided `out_error` struct.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_get_last_error_info(out_error: *mut TTZipErrorInfo) -> bool {
    if out_error.is_null() {
        return false;
    }
    LAST_ERROR.with(|cell| {
        let err = cell.borrow();
        if err.status == TTZipStatus::Ok && err.message[0] == 0 {
            false
        } else {
            (*out_error).struct_size = std::mem::size_of::<TTZipErrorInfo>() as u32;
            (*out_error).abi_version = TTZIP_ABI_VERSION_2;
            (*out_error).status = err.status;
            (*out_error).error_code = err.status as i32;
            (*out_error).offset = err.offset;
            for i in 0..512 {
                (*out_error).message[i] = err.message[i] as c_char;
            }
            for i in 0..256 {
                (*out_error).entry_path[i] = err.entry_path[i] as c_char;
            }
            true
        }
    })
}

/// Returns a heap-allocated, owned C-string copy of the last error message.
/// The caller must free it using `ttzip_free(ptr, TTZipMemoryKind::String)`.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_get_last_error_message_owned() -> *mut c_char {
    LAST_ERROR.with(|cell| {
        let err = cell.borrow();
        if err.status == TTZipStatus::Ok || err.message[0] == 0 {
            std::ptr::null_mut()
        } else {
            let len = err.message.iter().position(|&b| b == 0).unwrap_or(err.message.len());
            match std::ffi::CString::new(&err.message[..len]) {
                Ok(c_str) => c_str.into_raw(),
                Err(_) => std::ptr::null_mut(),
            }
        }
    })
}

#[deprecated(since = "2.0.0", note = "Raw TLS pointers are unsafe across threads. Use ttzip_rust_get_last_error_info or ttzip_rust_get_last_error_message_owned")]
pub fn get_last_error_message() -> *const c_char {
    LAST_ERROR.with(|cell| {
        let err = cell.borrow();
        if err.status == TTZipStatus::Ok || err.message[0] == 0 {
            std::ptr::null()
        } else {
            err.message.as_ptr() as *const c_char
        }
    })
}

pub fn clear_last_error() {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = DiagnosticErrorContext::empty();
    });
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TTZipEngineTag {
    Unknown = 0,
    RustRayonParallelZip = 1,
    RustStreamingParallelZip = 2,
    RustZeroCopy7zDecoder = 3,
    RustPure7zEncoder = 4,
    RustTarStreamEngine = 5,
    RustInPlaceZip = 6,
    RustInPlaceSevenZip = 7,
    RustVfsParallelScanner = 8,
    LibarchiveLegacy = 100,
    Cli7zFallback = 101,
    SystemTarFallback = 102,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TTZipExecutionProvenance {
    pub struct_size: u32,
    pub abi_version: u32,
    pub engine_tag: TTZipEngineTag,
    pub thread_count: u32,
    pub uncompressed_bytes: u64,
    pub compressed_bytes: u64,
    pub kernel_duration_nanos: u64,
    pub is_fallback: bool,
    pub fallback_reason: [c_char; 128],
}

impl Default for TTZipExecutionProvenance {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            abi_version: TTZIP_ABI_VERSION_2,
            engine_tag: TTZipEngineTag::Unknown,
            thread_count: 1,
            uncompressed_bytes: 0,
            compressed_bytes: 0,
            kernel_duration_nanos: 0,
            is_fallback: false,
            fallback_reason: [0; 128],
        }
    }
}

thread_local! {
    static LAST_PROVENANCE: RefCell<TTZipExecutionProvenance> = const { RefCell::new(TTZipExecutionProvenance {
        struct_size: std::mem::size_of::<TTZipExecutionProvenance>() as u32,
        abi_version: TTZIP_ABI_VERSION_2,
        engine_tag: TTZipEngineTag::Unknown,
        thread_count: 1,
        uncompressed_bytes: 0,
        compressed_bytes: 0,
        kernel_duration_nanos: 0,
        is_fallback: false,
        fallback_reason: [0; 128],
    }) };
}

pub fn record_execution_provenance(prov: TTZipExecutionProvenance) {
    LAST_PROVENANCE.with(|cell| {
        *cell.borrow_mut() = prov;
    });
}

pub fn get_execution_provenance(out: *mut TTZipExecutionProvenance) -> bool {
    if out.is_null() {
        return false;
    }
    LAST_PROVENANCE.with(|cell| unsafe {
        *out = *cell.borrow();
    });
    true
}

/// Zero-copy C-ABI packed array representing batch entries for FFI transfer.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TTZipPackedEntryArray {
    pub struct_size: u32,
    pub abi_version: u32,
    pub utf8_bytes: *const u8,
    pub total_bytes_len: usize,
    pub path_offsets: *const u32,
    pub path_lens: *const u32,
    pub uncompressed_sizes: *const u64,
    pub compressed_sizes: *const u64,
    pub crc32s: *const u32,
    pub mtimes: *const i64,
    pub modes: *const u32,
    pub flags: *const u8,
    pub count: usize,
}

/// Windowed VFS node summary DTO for zero-copy UI paging queries.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TTZipVfsNodeSummary {
    pub struct_size: u32,
    pub abi_version: u32,
    pub node_id: u32,
    pub name_utf8: *const c_char,
    pub name_len: u32,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub mtime_epoch_secs: i64,
    pub mode: u32,
    pub is_directory: bool,
    pub is_encrypted: bool,
    pub has_children: bool,
}


