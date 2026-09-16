// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! TTZip C-ABI Root Module strictly aligned with `sdk/include/ttzip.h`.
//!
//! Exposes version discovery, status code diagnostics, global lifecycle initialization,
//! last error management, engine tag identification, and dynamic string deallocation.

pub mod archive;
pub mod codecs;
pub mod crypto;
pub mod helpers;
pub mod split;
pub mod system;

/// Compatibility module mapping legacy archive_ffi guards, sys, and split.
pub mod archive_ffi {
    pub use crate::archive::{guards, sys};
    pub use crate::c_api::split;
}

/// Compatibility module mapping legacy codecs_ffi to c_api::codecs.
pub mod codecs_ffi {
    pub use crate::c_api::codecs::*;
}

pub use archive::*;
pub use codecs::*;
pub use crypto::*;
pub use helpers::*;
pub use split::*;
pub use system::*;

pub use crate::c_api::ttzip_rust_free_string as ttzip_free_string;

use crate::types::{clear_last_error, TTZipEngineTag, TTZipStatus};
use libc::c_char;
use std::ffi::CString;
use std::panic::catch_unwind;

// MARK: - Version & Initialization

/// Returns the semantic engine and runtime version string.
#[no_mangle]
pub extern "C" fn ttzip_rust_version() -> *const c_char {
    c"1.0.0-rust-engine".as_ptr()
}

/// Initializes TTZip Rust runtime and subsystem states.
#[no_mangle]
pub extern "C" fn ttzip_rust_init() -> TTZipStatus {
    let result = catch_unwind(|| TTZipStatus::Ok);
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

/// Converts a TTZipStatus code to a human-readable English string description.
#[no_mangle]
pub extern "C" fn ttzip_rust_status_string(status: TTZipStatus) -> *const c_char {
    match status {
        TTZipStatus::Ok => c"OK".as_ptr(),
        TTZipStatus::Eof => c"End of File".as_ptr(),
        TTZipStatus::Cancelled => c"Cancelled".as_ptr(),
        TTZipStatus::ErrInvalidParam => c"Invalid Parameter".as_ptr(),
        TTZipStatus::ErrFileNotFound => c"File Not Found".as_ptr(),
        TTZipStatus::ErrMmapFailed => c"Memory Map Failed".as_ptr(),
        TTZipStatus::ErrCorruptHeader => c"Corrupt Header".as_ptr(),
        TTZipStatus::ErrInvalidOffset => c"Invalid Offset".as_ptr(),
        TTZipStatus::ErrArchiveInitFailed => c"Archive Init Failed".as_ptr(),
        TTZipStatus::ErrOpenFailed => c"Open Failed".as_ptr(),
        TTZipStatus::ErrPathTooLong => c"Path Too Long".as_ptr(),
        TTZipStatus::ErrOutOfMemory => c"Out of Memory".as_ptr(),
        TTZipStatus::ErrInvalidPassword => c"Invalid Password / Authentication Failed".as_ptr(),
        TTZipStatus::ErrExtractionFailed => c"Extraction Failed".as_ptr(),
        TTZipStatus::ErrCompressionFailed => c"Compression Failed".as_ptr(),
        TTZipStatus::ErrSolidBudgetExceeded => c"Solid Budget Exceeded".as_ptr(),
        TTZipStatus::ErrSecurityViolation => c"Security Violation".as_ptr(),
        TTZipStatus::ErrUnsupportedFeature => c"Unsupported Feature".as_ptr(),
        TTZipStatus::ErrPanicCaught => c"Panic Caught".as_ptr(),
    }
}

/// Returns true if hardware acceleration (ARM64 NEON / Crypto or x86 AES-NI / AVX2) is active.
#[no_mangle]
pub extern "C" fn ttzip_rust_is_hardware_accelerated() -> bool {
    #[cfg(target_arch = "aarch64")]
    {
        true
    }
    #[cfg(target_arch = "x86_64")]
    {
        is_x86_feature_detected!("aes") && is_x86_feature_detected!("avx2")
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        false
    }
}

// MARK: - Last Error Diagnostics

/// Returns thread-local diagnostic error message or NULL if previous operation succeeded.
#[no_mangle]
#[allow(deprecated)]
pub extern "C" fn ttzip_rust_last_error_message() -> *const c_char {
    crate::types::get_last_error_message()
}

/// Clears thread-local diagnostic error message.
#[no_mangle]
pub extern "C" fn ttzip_rust_clear_last_error() {
    clear_last_error();
}

pub use crate::types::{ttzip_rust_get_last_error_info, ttzip_rust_get_last_error_message_owned};

// MARK: - Memory & Engine Tag

/// Frees a heap-allocated C string previously returned by TTZip C-ABI routines.
///
/// # Safety
/// If `ptr` is non-null, it must have been allocated by TTZip C-ABI via `CString::into_raw`.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

/// Returns the string representation of an engine execution tag.
#[no_mangle]
pub extern "C" fn ttzip_rust_engine_tag_name(tag: TTZipEngineTag) -> *const c_char {
    match tag {
        TTZipEngineTag::RustRayonParallelZip => c"RustRayonParallelZip".as_ptr(),
        TTZipEngineTag::RustStreamingParallelZip => c"RustStreamingParallelZip".as_ptr(),
        TTZipEngineTag::RustZeroCopy7zDecoder => c"RustZeroCopy7zDecoder".as_ptr(),
        TTZipEngineTag::RustPure7zEncoder => c"RustPure7zEncoder".as_ptr(),
        TTZipEngineTag::RustTarStreamEngine => c"RustTarStreamEngine".as_ptr(),
        TTZipEngineTag::RustInPlaceZip => c"RustInPlaceZip".as_ptr(),
        TTZipEngineTag::RustInPlaceSevenZip => c"RustInPlaceSevenZip".as_ptr(),
        TTZipEngineTag::RustVfsParallelScanner => c"RustVfsParallelScanner".as_ptr(),
        TTZipEngineTag::LibarchiveLegacy => c"LibarchiveLegacy".as_ptr(),
        TTZipEngineTag::Cli7zFallback => c"Cli7zFallback".as_ptr(),
        TTZipEngineTag::SystemTarFallback => c"SystemTarFallback".as_ptr(),
        TTZipEngineTag::Unknown => c"Unknown".as_ptr(),
    }
}
