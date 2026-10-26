// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Structured logging router forwarding Rust `log` records to Swift `TTLogger` C callback.

use crate::types::{TTZipLogLevel, TTZipStatus};
use libc::{c_char, c_void};
use log::{Level, LevelFilter, Metadata, Record};
use std::ffi::CString;
use std::panic::catch_unwind;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU8, Ordering};

/// C-ABI callback for log dispatch.
pub type TTZipLogCallback = Option<
    unsafe extern "C" fn(
        level: TTZipLogLevel,
        target_module: *const c_char,
        message: *const c_char,
        file: *const c_char,
        line: i32,
        user_data: *mut c_void,
    ),
>;

struct LoggerGlobalState {
    callback: AtomicPtr<c_void>,
    user_data: AtomicPtr<c_void>,
    min_level: AtomicU8,
    is_installed: AtomicBool,
}

static LOGGER_STATE: LoggerGlobalState = LoggerGlobalState {
    callback: AtomicPtr::new(std::ptr::null_mut()),
    user_data: AtomicPtr::new(std::ptr::null_mut()),
    min_level: AtomicU8::new(TTZipLogLevel::Info as u8),
    is_installed: AtomicBool::new(false),
};

/// Logger implementation bridging Rust `log` crate to C-ABI callback.
pub struct TTZipLogger;

impl log::Log for TTZipLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        let min_lvl = LOGGER_STATE.min_level.load(Ordering::Acquire);
        let record_lvl = match metadata.level() {
            Level::Error => TTZipLogLevel::Error as u8,
            Level::Warn => TTZipLogLevel::Warning as u8,
            Level::Info => TTZipLogLevel::Info as u8,
            Level::Debug | Level::Trace => TTZipLogLevel::Debug as u8,
        };
        record_lvl >= min_lvl
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let cb_ptr = LOGGER_STATE.callback.load(Ordering::Acquire);
        if cb_ptr.is_null() {
            return;
        }

        let user_data = LOGGER_STATE.user_data.load(Ordering::Acquire);
        let level = match record.level() {
            Level::Error => TTZipLogLevel::Error,
            Level::Warn => TTZipLogLevel::Warning,
            Level::Info => TTZipLogLevel::Info,
            Level::Debug | Level::Trace => TTZipLogLevel::Debug,
        };

        let target_str = record.target();
        let msg_str = format!("{}", record.args());
        let file_str = record.file().unwrap_or("");
        let line_num = record.line().unwrap_or(0) as i32;

        let target_c = CString::new(target_str).unwrap_or_default();
        let msg_c = CString::new(msg_str).unwrap_or_default();
        let file_c = CString::new(file_str).unwrap_or_default();

        let cb: unsafe extern "C" fn(
            TTZipLogLevel,
            *const c_char,
            *const c_char,
            *const c_char,
            i32,
            *mut c_void,
        ) = unsafe { std::mem::transmute(cb_ptr) };

        let _ = catch_unwind(|| unsafe {
            cb(
                level,
                target_c.as_ptr(),
                msg_c.as_ptr(),
                file_c.as_ptr(),
                line_num,
                user_data,
            );
        });
    }

    fn flush(&self) {}
}

static STATIC_LOGGER: TTZipLogger = TTZipLogger;

/// Configures global logger callback and minimum log level.
pub fn set_logger_callback(
    callback: TTZipLogCallback,
    min_level: TTZipLogLevel,
    user_data: *mut c_void,
) -> TTZipStatus {
    let cb_ptr = match callback {
        Some(cb) => cb as *mut c_void,
        None => std::ptr::null_mut(),
    };

    LOGGER_STATE.callback.store(cb_ptr, Ordering::Release);
    LOGGER_STATE.user_data.store(user_data, Ordering::Release);
    LOGGER_STATE.min_level.store(min_level as u8, Ordering::Release);

    if !LOGGER_STATE.is_installed.swap(true, Ordering::SeqCst) {
        let max_filter = match min_level {
            TTZipLogLevel::Debug => LevelFilter::Debug,
            TTZipLogLevel::Info => LevelFilter::Info,
            TTZipLogLevel::Warning => LevelFilter::Warn,
            TTZipLogLevel::Error => LevelFilter::Error,
        };
        let _ = log::set_logger(&STATIC_LOGGER);
        log::set_max_level(max_filter);
        install_tracing_bridge();
    } else {
        let max_filter = match min_level {
            TTZipLogLevel::Debug => LevelFilter::Debug,
            TTZipLogLevel::Info => LevelFilter::Info,
            TTZipLogLevel::Warning => LevelFilter::Warn,
            TTZipLogLevel::Error => LevelFilter::Error,
        };
        log::set_max_level(max_filter);
    }

    install_tracing_bridge();

    TTZipStatus::Ok
}

static TRACING_INSTALLED: AtomicBool = AtomicBool::new(false);

fn install_tracing_bridge() {
    if !TRACING_INSTALLED.swap(true, Ordering::SeqCst) {
        let subscriber = TTZipTracingSubscriber::new();
        let _ = tracing::subscriber::set_global_default(subscriber);
    }
}

use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::time::Instant;
use parking_lot::Mutex;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record as SpanRecord};
use tracing::{Event, Level as TracingLevel, Metadata as TracingMetadata, Subscriber};

struct ActiveSpan {
    name: &'static str,
    start: Instant,
}

/// Lightweight tracing subscriber routing spans and events into the native C-ABI logger.
pub struct TTZipTracingSubscriber {
    next_id: AtomicU64,
    spans: Mutex<HashMap<u64, ActiveSpan>>,
}

impl TTZipTracingSubscriber {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            spans: Mutex::new(HashMap::with_capacity(32)),
        }
    }
}

impl Default for TTZipTracingSubscriber {
    fn default() -> Self {
        Self::new()
    }
}

struct EventMessageVisitor<'a>(&'a mut String);

impl<'a> Visit for EventMessageVisitor<'a> {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        use std::fmt::Write;
        if field.name() == "message" {
            let _ = write!(self.0, "{:?}", value);
        } else {
            let _ = write!(self.0, " {}={:?}", field.name(), value);
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.0.push_str(value);
        } else {
            self.0.push(' ');
            self.0.push_str(field.name());
            self.0.push('=');
            self.0.push_str(value);
        }
    }
}

impl Subscriber for TTZipTracingSubscriber {
    fn enabled(&self, metadata: &TracingMetadata<'_>) -> bool {
        let min_lvl = LOGGER_STATE.min_level.load(Ordering::Acquire);
        let record_lvl = match *metadata.level() {
            TracingLevel::ERROR => TTZipLogLevel::Error as u8,
            TracingLevel::WARN => TTZipLogLevel::Warning as u8,
            TracingLevel::INFO => TTZipLogLevel::Info as u8,
            TracingLevel::DEBUG | TracingLevel::TRACE => TTZipLogLevel::Debug as u8,
        };
        record_lvl >= min_lvl
    }

    fn new_span(&self, span: &Attributes<'_>) -> Id {
        let id_val = self.next_id.fetch_add(1, Ordering::Relaxed);
        let span_id = Id::from_u64(id_val);
        let name = span.metadata().name();
        
        {
            let mut guard = self.spans.lock();
            guard.insert(
                id_val,
                ActiveSpan {
                    name,
                    start: Instant::now(),
                },
            );
        }

        let min_lvl = LOGGER_STATE.min_level.load(Ordering::Acquire);
        if (TTZipLogLevel::Debug as u8) >= min_lvl {
            let target = span.metadata().target();
            let msg = format!("[SPAN:START] name=\"{}\" id={}", name, id_val);
            let file = span.metadata().file().unwrap_or("tracing");
            let line = span.metadata().line().unwrap_or(0) as i32;
            emit_log_direct(TTZipLogLevel::Debug, target, &msg, file, line);
        }

        span_id
    }

    fn record(&self, _span: &Id, _values: &SpanRecord<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        if !self.enabled(event.metadata()) {
            return;
        }

        let level = match *event.metadata().level() {
            TracingLevel::ERROR => TTZipLogLevel::Error,
            TracingLevel::WARN => TTZipLogLevel::Warning,
            TracingLevel::INFO => TTZipLogLevel::Info,
            TracingLevel::DEBUG | TracingLevel::TRACE => TTZipLogLevel::Debug,
        };

        let mut msg_buf = String::with_capacity(128);
        event.record(&mut EventMessageVisitor(&mut msg_buf));

        let target = event.metadata().target();
        let file = event.metadata().file().unwrap_or("tracing");
        let line = event.metadata().line().unwrap_or(0) as i32;

        emit_log_direct(level, target, &msg_buf, file, line);
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, span: &Id) {
        let id_val = span.into_u64();
        let removed = {
            let mut guard = self.spans.lock();
            guard.remove(&id_val)
        };

        if let Some(active) = removed {
            let min_lvl = LOGGER_STATE.min_level.load(Ordering::Acquire);
            if (TTZipLogLevel::Debug as u8) >= min_lvl {
                let duration_us = active.start.elapsed().as_micros();
                let msg = format!(
                    "[SPAN:FINISH] name=\"{}\" id={} duration_us={}",
                    active.name, id_val, duration_us
                );
                emit_log_direct(TTZipLogLevel::Debug, "tracing::span", &msg, "tracing", 0);
            }
        }
    }
}

/// Directly sends a log event to the registered C callback.
pub fn emit_log_direct(
    level: TTZipLogLevel,
    target: &str,
    message: &str,
    file: &str,
    line: i32,
) {
    let cb_ptr = LOGGER_STATE.callback.load(Ordering::Acquire);
    if cb_ptr.is_null() {
        return;
    }

    let min_lvl = LOGGER_STATE.min_level.load(Ordering::Acquire);
    if (level as u8) < min_lvl {
        return;
    }

    let user_data = LOGGER_STATE.user_data.load(Ordering::Acquire);
    let target_c = CString::new(target).unwrap_or_default();
    let msg_c = CString::new(message).unwrap_or_default();
    let file_c = CString::new(file).unwrap_or_default();

    let cb: unsafe extern "C" fn(
        TTZipLogLevel,
        *const c_char,
        *const c_char,
        *const c_char,
        i32,
        *mut c_void,
    ) = unsafe { std::mem::transmute(cb_ptr) };

    let _ = catch_unwind(|| unsafe {
        cb(
            level,
            target_c.as_ptr(),
            msg_c.as_ptr(),
            file_c.as_ptr(),
            line,
            user_data,
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    static LOG_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "C" fn test_log_sink(
        level: TTZipLogLevel,
        target: *const c_char,
        msg: *const c_char,
        _file: *const c_char,
        _line: i32,
        _user_data: *mut c_void,
    ) {
        let target_str = std::ffi::CStr::from_ptr(target).to_str().unwrap();
        if target_str == "test::module" {
            LOG_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
            assert_eq!(level, TTZipLogLevel::Warning);
            let msg_str = std::ffi::CStr::from_ptr(msg).to_str().unwrap();
            assert_eq!(msg_str, "Disk space low warning");
        }
    }

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_logger_routing_to_c_callback() {
        let _guard = TEST_LOCK.lock();
        LOG_CALL_COUNT.store(0, Ordering::SeqCst);
        let status = set_logger_callback(Some(test_log_sink), TTZipLogLevel::Debug, std::ptr::null_mut());
        assert_eq!(status, TTZipStatus::Ok);

        emit_log_direct(
            TTZipLogLevel::Warning,
            "test::module",
            "Disk space low warning",
            "logging.rs",
            42,
        );

        assert_eq!(LOG_CALL_COUNT.load(Ordering::SeqCst), 1);
    }

    static TRACING_EVENT_COUNT: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "C" fn tracing_test_sink(
        _level: TTZipLogLevel,
        target: *const c_char,
        msg: *const c_char,
        _file: *const c_char,
        _line: i32,
        _user_data: *mut c_void,
    ) {
        if target.is_null() || msg.is_null() {
            return;
        }
        let target_str = std::ffi::CStr::from_ptr(target).to_str().unwrap_or("");
        let msg_str = std::ffi::CStr::from_ptr(msg).to_str().unwrap_or("");
        if target_str.contains("tracing_test") || msg_str.contains("tracing test event") || msg_str.contains("test_span") {
            TRACING_EVENT_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn test_tracing_event_and_span_routing() {
        let _guard = TEST_LOCK.lock();
        TRACING_EVENT_COUNT.store(0, Ordering::SeqCst);
        let status = set_logger_callback(Some(tracing_test_sink), TTZipLogLevel::Debug, std::ptr::null_mut());
        assert_eq!(status, TTZipStatus::Ok);

        tracing::info!(target: "tracing_test", "tracing test event dispatched");
        let span = tracing::info_span!("test_span");
        let _span_guard = span.enter();
        drop(_span_guard);

        assert!(TRACING_EVENT_COUNT.load(Ordering::SeqCst) >= 1);
    }
}

