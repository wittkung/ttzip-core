// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Runtime synchronization, lock-free ring buffer, and worker pool FFI exports.

pub mod cancellation_ffi;
pub mod logging_ffi;

pub use cancellation_ffi::*;
pub use logging_ffi::*;

use std::ffi::c_char;
use std::panic::catch_unwind;
use crate::types::TTZipStatus;

/// Returns current semantic engine and runtime version string.
#[no_mangle]
pub extern "C" fn ttzip_rust_version() -> *const c_char {
    c"1.0.0-rust-engine".as_ptr()
}

/// Initializes TTZip Rust runtime and subsystem states.
#[no_mangle]
pub extern "C" fn ttzip_rust_init() -> TTZipStatus {
    let result = catch_unwind(|| {
        TTZipStatus::Ok
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

/// Converts a TTZipStatus code to a human-readable English string description.
#[no_mangle]
pub extern "C" fn ttzip_rust_status_string(status: TTZipStatus) -> *const c_char {
    match status {
        TTZipStatus::Ok => c"OK".as_ptr(),
        TTZipStatus::Eof => c"EOF".as_ptr(),
        TTZipStatus::Cancelled => c"Cancelled".as_ptr(),
        TTZipStatus::ErrInvalidParam => c"Invalid Parameter".as_ptr(),
        TTZipStatus::ErrFileNotFound => c"File Not Found".as_ptr(),
        TTZipStatus::ErrMmapFailed => c"Mmap Failed".as_ptr(),
        TTZipStatus::ErrCorruptHeader => c"Corrupt Header".as_ptr(),
        TTZipStatus::ErrInvalidOffset => c"Invalid Offset".as_ptr(),
        TTZipStatus::ErrArchiveInitFailed => c"Archive Init Failed".as_ptr(),
        TTZipStatus::ErrOpenFailed => c"Open Failed".as_ptr(),
        TTZipStatus::ErrPathTooLong => c"Path Too Long".as_ptr(),
        TTZipStatus::ErrOutOfMemory => c"Out Of Memory".as_ptr(),
        TTZipStatus::ErrInvalidPassword => c"Invalid Password".as_ptr(),
        TTZipStatus::ErrExtractionFailed => c"Extraction Failed".as_ptr(),
        TTZipStatus::ErrCompressionFailed => c"Compression Failed".as_ptr(),
        TTZipStatus::ErrSolidBudgetExceeded => c"Solid Budget Exceeded".as_ptr(),
        TTZipStatus::ErrSecurityViolation => c"Security Violation".as_ptr(),
        TTZipStatus::ErrUnsupportedFeature => c"Unsupported Feature".as_ptr(),
        TTZipStatus::ErrPanicCaught => c"Panic Caught".as_ptr(),
    }

}

/// Returns true if hardware acceleration (ARM64 NEON / Crypto extensions) is active.
#[no_mangle]
pub extern "C" fn ttzip_rust_is_hardware_accelerated() -> bool {
    #[cfg(target_arch = "aarch64")]
    {
        true
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        false
    }
}
