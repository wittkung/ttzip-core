// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

use std::ffi::CStr;
use ttzip_engine::ffi::{ttzip_rust_clear_last_error, ttzip_rust_last_error_message};
use ttzip_engine::types::{set_last_error, TTZipStatus};

#[test]
fn test_error_diagnostics_thread_local() {
    // 1. Initially clear
    ttzip_rust_clear_last_error();
    assert!(ttzip_rust_last_error_message().is_null());

    // 2. Set diagnostic error
    set_last_error(
        TTZipStatus::ErrCorruptHeader,
        "Corrupt local header at offset 0x1A40: entry 'data/test.bin' crc mismatch",
        Some("data/test.bin"),
        0x1A40,
    );

    let msg_ptr = ttzip_rust_last_error_message();
    assert!(!msg_ptr.is_null());
    let msg_str = unsafe { CStr::from_ptr(msg_ptr).to_str().unwrap() };
    assert!(msg_str.contains("Corrupt local header"));
    assert!(msg_str.contains("0x1A40"));

    // 3. Clear
    ttzip_rust_clear_last_error();
    assert!(ttzip_rust_last_error_message().is_null());
}
