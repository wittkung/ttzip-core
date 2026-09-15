// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI Structured Logging Export Layer.
//!
//! Bridges native Rust `log` records and microkernel telemetry directly
//! to host callbacks (e.g. Swift `TTLogger`) across the FFI boundary.

use crate::runtime::logging::{emit_log_direct, set_logger_callback};
use crate::types::{TTZipLogLevel, TTZipStatus};
use crate::uniffi_api::types::TTZipError;
use libc::{c_char, c_void};
use parking_lot::RwLock;
use std::ffi::CStr;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

/// Host logging callback interface invoked when the microkernel emits a record.
#[uniffi::export(callback_interface)]
pub trait UniFFILogCallback: Send + Sync {
    fn log(&self, level: u32, target: String, message: String, file: String, line: u32);
}

static UNIFFI_LOG_SINK: RwLock<Option<Arc<dyn UniFFILogCallback>>> = RwLock::new(None);

unsafe extern "C" fn uniffi_log_forwarder_c(
    level: TTZipLogLevel,
    target_module: *const c_char,
    message: *const c_char,
    file: *const c_char,
    line: i32,
    _user_data: *mut c_void,
) {
    let callback = {
        let guard = UNIFFI_LOG_SINK.read();
        guard.clone()
    };

    if let Some(cb) = callback {
        let target = if !target_module.is_null() {
            CStr::from_ptr(target_module).to_string_lossy().into_owned()
        } else {
            String::new()
        };

        let msg = if !message.is_null() {
            CStr::from_ptr(message).to_string_lossy().into_owned()
        } else {
            String::new()
        };

        let file_path = if !file.is_null() {
            CStr::from_ptr(file).to_string_lossy().into_owned()
        } else {
            String::new()
        };

        let line_num = if line >= 0 { line as u32 } else { 0 };
        let lvl = level as u32;

        let _ = catch_unwind(AssertUnwindSafe(|| {
            cb.log(lvl, target, msg, file_path, line_num);
        }));
    }
}

/// Configures or clears the active UniFFI logging sink and adjusts the minimum severity threshold.
#[uniffi::export]
pub fn uniffi_set_logger(
    callback: Option<Box<dyn UniFFILogCallback>>,
    min_level: u32,
) -> Result<(), TTZipError> {
    let internal_level = match min_level {
        0 => TTZipLogLevel::Debug,
        1 => TTZipLogLevel::Info,
        2 => TTZipLogLevel::Warning,
        _ => TTZipLogLevel::Error,
    };

    match callback {
        Some(cb) => {
            let arc_cb: Arc<dyn UniFFILogCallback> = Arc::from(cb);
            {
                let mut guard = UNIFFI_LOG_SINK.write();
                *guard = Some(arc_cb);
            }
            let status = set_logger_callback(
                Some(uniffi_log_forwarder_c),
                internal_level,
                std::ptr::null_mut(),
            );
            if status != TTZipStatus::Ok {
                return Err(TTZipError::EngineError {
                    code: status as i32,
                });
            }
        }
        None => {
            {
                let mut guard = UNIFFI_LOG_SINK.write();
                *guard = None;
            }
            let status = set_logger_callback(None, internal_level, std::ptr::null_mut());
            if status != TTZipStatus::Ok {
                return Err(TTZipError::EngineError {
                    code: status as i32,
                });
            }
        }
    }

    Ok(())
}

/// Directly emits a structured log event into the engine router across the UniFFI boundary.
#[uniffi::export]
pub fn uniffi_log_direct(
    level: u32,
    target: String,
    message: String,
    file: String,
    line: u32,
) {
    let log_level = match level {
        0 => TTZipLogLevel::Debug,
        1 => TTZipLogLevel::Info,
        2 => TTZipLogLevel::Warning,
        _ => TTZipLogLevel::Error,
    };
    emit_log_direct(log_level, &target, &message, &file, line as i32);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestLogSink {
        call_count: Arc<AtomicUsize>,
    }

    impl UniFFILogCallback for TestLogSink {
        fn log(&self, level: u32, target: String, message: String, file: String, line: u32) {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            assert_eq!(level, 1);
            assert_eq!(target, "test::uniffi");
            assert_eq!(message, "UniFFI structured logging verified");
            assert_eq!(file, "logging.rs");
            assert_eq!(line, 100);
        }
    }

    #[test]
    fn test_uniffi_logging_roundtrip() {
        let counter = Arc::new(AtomicUsize::new(0));
        let sink = Box::new(TestLogSink {
            call_count: counter.clone(),
        });

        let res = uniffi_set_logger(Some(sink), 0);
        assert!(res.is_ok());

        uniffi_log_direct(
            1,
            "test::uniffi".to_string(),
            "UniFFI structured logging verified".to_string(),
            "logging.rs".to_string(),
            100,
        );

        assert_eq!(counter.load(Ordering::SeqCst), 1);

        let clear_res = uniffi_set_logger(None, 1);
        assert!(clear_res.is_ok());
    }
}
