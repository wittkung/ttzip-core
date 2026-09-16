// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! C-ABI FFI exports for runtime cancellation tokens.

use crate::runtime::cancellation::{CancellationReason, CancellationToken};
use std::panic::catch_unwind;
use std::sync::Arc;

#[no_mangle]
pub extern "C" fn ttzip_rust_cancellation_token_new() -> *mut CancellationToken {
    let result = catch_unwind(|| {
        let token = Arc::new(CancellationToken::new());
        Arc::into_raw(token) as *mut CancellationToken
    });
    result.unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_cancellation_token_retain(token: *const CancellationToken) {
    let _ = catch_unwind(|| {
        if !token.is_null() {
            Arc::increment_strong_count(token);
        }
    });
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_cancellation_token_cancel(
    token: *mut CancellationToken,
    reason: u8,
) {
    let _ = catch_unwind(|| {
        if !token.is_null() {
            (*token).cancel(CancellationReason::from_u8(reason));
        }
    });
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_cancellation_token_is_cancelled(
    token: *const CancellationToken,
) -> bool {
    let result = catch_unwind(|| {
        if token.is_null() {
            false
        } else {
            (*token).is_cancelled()
        }
    });
    result.unwrap_or(false)
}

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_cancellation_token_free(token: *const CancellationToken) {
    let _ = catch_unwind(|| {
        if !token.is_null() {
            Arc::decrement_strong_count(token);
        }
    });
}
