// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Core status codes, error models, and thread-local diagnostic channels.

use std::cell::RefCell;
use libc::c_char;

use super::options::TTZIP_ABI_VERSION_2;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    ErrUnsupportedFeature = -13,
    ErrSolidBudgetExceeded = -24,
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
            TTZipStatus::ErrUnsupportedFeature => "Unsupported compression algorithm or archive feature",
            TTZipStatus::ErrSolidBudgetExceeded => "7z solid decompression budget exceeded",
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
