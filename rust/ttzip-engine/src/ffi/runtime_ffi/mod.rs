// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Runtime synchronization, lock-free ring buffer, and worker pool FFI exports.

pub mod cancellation_ffi;
pub mod logging_ffi;

pub use cancellation_ffi::*;
pub use logging_ffi::*;

use std::ffi::c_char;
use std::panic::catch_unwind;
use crate::types::TTZipStatus;

/// Returns current semantic engine and runtime version string.
#[unsafe(no_mangle)]
pub extern "C" fn ttzip_rust_version() -> *const c_char {
    c"1.0.0-rust-engine".as_ptr()
}

/// Initializes TTZip Rust runtime and subsystem states.
#[unsafe(no_mangle)]
pub extern "C" fn ttzip_rust_init() -> TTZipStatus {
    let result = catch_unwind(|| {
        TTZipStatus::Ok
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

fn operation_status_string(status: TTZipStatus) -> Option<*const c_char> {
    match status {
        TTZipStatus::Ok => Some(c"OK".as_ptr()),
        TTZipStatus::Eof => Some(c"EOF".as_ptr()),
        TTZipStatus::Cancelled => Some(c"Cancelled".as_ptr()),
        TTZipStatus::ErrInvalidParam => Some(c"Invalid Parameter".as_ptr()),
        TTZipStatus::ErrFileNotFound => Some(c"File Not Found".as_ptr()),
        TTZipStatus::ErrMmapFailed => Some(c"Mmap Failed".as_ptr()),
        TTZipStatus::ErrCorruptHeader => Some(c"Corrupt Header".as_ptr()),
        TTZipStatus::ErrInvalidOffset => Some(c"Invalid Offset".as_ptr()),
        TTZipStatus::ErrArchiveInitFailed => Some(c"Archive Init Failed".as_ptr()),
        TTZipStatus::ErrOpenFailed => Some(c"Open Failed".as_ptr()),
        TTZipStatus::ErrPathTooLong => Some(c"Path Too Long".as_ptr()),
        _ => None,
    }
}

/// Converts a TTZipStatus code to a human-readable English string description.
#[unsafe(no_mangle)]
pub extern "C" fn ttzip_rust_status_string(status: TTZipStatus) -> *const c_char {
    if let Some(ptr) = operation_status_string(status) {
        return ptr;
    }
    match status {
        TTZipStatus::ErrOutOfMemory => c"Out Of Memory".as_ptr(),
        TTZipStatus::ErrInvalidPassword => c"Invalid Password".as_ptr(),
        TTZipStatus::ErrExtractionFailed => c"Extraction Failed".as_ptr(),
        TTZipStatus::ErrCompressionFailed => c"Compression Failed".as_ptr(),
        TTZipStatus::ErrSolidBudgetExceeded => c"Solid Budget Exceeded".as_ptr(),
        TTZipStatus::ErrSecurityViolation => c"Security Violation".as_ptr(),
        TTZipStatus::ErrUnsupportedFeature => c"Unsupported Feature".as_ptr(),
        TTZipStatus::ErrBufferTooSmall => c"Buffer Too Small".as_ptr(),
        TTZipStatus::ErrIo => c"I/O Error".as_ptr(),
        TTZipStatus::ErrPanicCaught => c"Panic Caught".as_ptr(),
        TTZipStatus::ErrAbiMismatch => c"ABI Mismatch".as_ptr(),
        _ => c"Unknown Status".as_ptr(),
    }
}

/// Returns true if hardware acceleration (ARM64 NEON / Crypto extensions) is active.
#[unsafe(no_mangle)]
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
