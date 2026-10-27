// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Bridges foreign progress callbacks and cancellation tokens to the C progress
//! hook (`TTZipExtractOptions::progress_callback`) consumed by every decoder.
//!
//! `progress_trampoline::<O>` is monomorphized per observer type, so the hook
//! itself performs no dynamic dispatch; the entry name is materialized as a
//! `String` only when a foreign handler is actually installed.

use std::ffi::{c_char, c_void, CStr};

use super::callback::{UniFFICancellationToken, UniFFIProgressCallback};
use super::types::{CancellationToken, ProgressHandler};

/// Receives decoder progress during `ExtractionSession` staging.
///
/// Decoders may invoke the hook from worker threads, hence `Sync`.
pub(crate) trait StageObserver: Sync {
    /// Returns true once the caller has requested cancellation.
    fn is_cancelled(&self) -> bool;
    /// Forwards one progress event. Returning false cancels the extraction.
    fn on_progress(&self, processed: u64, total: u64, entry: Option<&CStr>) -> bool;
}

/// C progress hook installed by the session; `user_data` points at an `O`.
///
/// # Safety
/// `user_data` must point to an `O` that stays alive for the whole extraction
/// call, and `entry` must be null or a NUL-terminated string valid for this call.
pub(crate) unsafe extern "C" fn progress_trampoline<O: StageObserver>(
    processed: u64,
    total: u64,
    entry: *const c_char,
    user_data: *mut c_void,
) -> bool {
    // SAFETY: the session sets `user_data` to `&O` and keeps the observer borrowed
    // for the duration of the synchronous extraction call that invokes this hook.
    let observer = unsafe { &*(user_data as *const O) };
    if observer.is_cancelled() {
        return false;
    }
    // SAFETY: decoders pass either null or a NUL-terminated entry path that
    // outlives this callback invocation.
    let entry = (!entry.is_null()).then(|| unsafe { CStr::from_ptr(entry) });
    observer.on_progress(processed, total, entry)
}

#[inline]
fn entry_name(entry: Option<&CStr>) -> Option<String> {
    entry.map(|e| e.to_string_lossy().into_owned())
}

/// Observer for the interactive session API (`ExtractionSession::stage`).
pub(crate) struct SessionStageObserver<'a> {
    pub progress: Option<&'a dyn UniFFIProgressCallback>,
    pub cancel: Option<&'a UniFFICancellationToken>,
}

impl StageObserver for SessionStageObserver<'_> {
    fn is_cancelled(&self) -> bool {
        self.cancel.is_some_and(UniFFICancellationToken::is_cancelled)
    }

    fn on_progress(&self, processed: u64, total: u64, entry: Option<&CStr>) -> bool {
        self.progress
            .is_none_or(|p| p.on_progress(processed, total, entry_name(entry)))
    }
}

/// Observer for the non-interactive one-shot functions in `extraction`.
pub(crate) struct OneShotStageObserver<'a> {
    pub progress: Option<&'a dyn ProgressHandler>,
    pub cancel: Option<&'a CancellationToken>,
}

impl StageObserver for OneShotStageObserver<'_> {
    fn is_cancelled(&self) -> bool {
        self.cancel.is_some_and(CancellationToken::is_cancelled)
    }

    fn on_progress(&self, processed: u64, total: u64, entry: Option<&CStr>) -> bool {
        self.progress
            .is_none_or(|p| p.on_progress(processed, total, entry_name(entry)))
    }
}
