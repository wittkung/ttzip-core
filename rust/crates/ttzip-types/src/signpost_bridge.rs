// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! 24-byte telemetry trace context and lock-free `os_signpost` interval tracing.
//!
//! # Architecture & C-ABI Invariants
//! - `TTLogTraceContext` is exactly 24 bytes, 8-byte aligned (`#[repr(C, align(8))]`).
//! - Thread-local lock-free context propagation with automatic RAII restoration.
//! - Direct bridge for distributed cross-XPC 64-bit `trace_id` and `span_id` propagation.

use core::cell::Cell;
use core::fmt;

/// 24-byte binary-compact telemetry trace context for cross-process propagation.
#[repr(C, align(8))]
#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub struct TTLogTraceContext {
    pub trace_id: u64,     // 0..8:   Distributed globally unique trace identifier
    pub span_id: u64,      // 8..16:  Current execution span identifier
    pub flags: u8,         // 16..17: Trace sampling and logging flags
    pub _reserved: [u8; 7],// 17..24: Explicit zero padding to 8-byte boundary
}

// Compile-time static assertions ensuring exact geometry matching C-ABI v6.0
const _: () = {
    assert!(core::mem::size_of::<TTLogTraceContext>() == 24);
    assert!(core::mem::align_of::<TTLogTraceContext>() == 8);
};

impl fmt::Debug for TTLogTraceContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TTLogTraceContext")
            .field("trace_id", &format_args!("0x{:016x}", self.trace_id))
            .field("span_id", &format_args!("0x{:016x}", self.span_id))
            .field("flags", &format_args!("0x{:02x}", self.flags))
            .finish()
    }
}

impl TTLogTraceContext {
    #[inline]
    pub const fn new(trace_id: u64, span_id: u64, flags: u8) -> Self {
        Self {
            trace_id,
            span_id,
            flags,
            _reserved: [0u8; 7],
        }
    }

    #[inline]
    pub const fn is_sampled(&self) -> bool {
        (self.flags & 0x01) != 0
    }
}

thread_local! {
    static THREAD_TRACE_CONTEXT: Cell<TTLogTraceContext> = const { Cell::new(TTLogTraceContext {
        trace_id: 0,
        span_id: 0,
        flags: 0,
        _reserved: [0u8; 7],
    }) };
}

/// Retrieve the current thread's trace context.
#[inline]
pub fn current_trace_context() -> TTLogTraceContext {
    THREAD_TRACE_CONTEXT.with(|c| c.get())
}

/// Set the current thread's trace context.
#[inline]
pub fn set_trace_context(ctx: TTLogTraceContext) {
    THREAD_TRACE_CONTEXT.with(|c| c.set(ctx));
}

/// RAII Guard that restores the previous trace context upon drop.
pub struct TraceScopeGuard {
    prev: TTLogTraceContext,
}

impl TraceScopeGuard {
    #[inline]
    pub fn enter(new_ctx: TTLogTraceContext) -> Self {
        let prev = current_trace_context();
        set_trace_context(new_ctx);
        Self { prev }
    }
}

impl Drop for TraceScopeGuard {
    #[inline]
    fn drop(&mut self) {
        set_trace_context(self.prev);
    }
}

/// Lightweight Signpost interval token.
#[derive(Debug, Copy, Clone)]
pub struct SignpostInterval {
    pub id: u64,
    pub name: &'static str,
    pub start_time_ns: u64,
}

impl SignpostInterval {
    #[inline]
    pub fn begin(name: &'static str, id: u64) -> Self {
        let start_time_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        Self {
            id,
            name,
            start_time_ns,
        }
    }

    #[inline]
    pub fn end(self) -> u64 {
        let end_time_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        end_time_ns.saturating_sub(self.start_time_ns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_context_geometry_and_alignment() {
        assert_eq!(core::mem::size_of::<TTLogTraceContext>(), 24);
        assert_eq!(core::mem::align_of::<TTLogTraceContext>(), 8);
    }

    #[test]
    fn test_thread_local_trace_context_and_scope_guard() {
        let root_ctx = TTLogTraceContext::new(0x1000, 0x2000, 0x01);
        set_trace_context(root_ctx);
        assert_eq!(current_trace_context(), root_ctx);

        {
            let child_ctx = TTLogTraceContext::new(0x1000, 0x2001, 0x01);
            let _guard = TraceScopeGuard::enter(child_ctx);
            assert_eq!(current_trace_context(), child_ctx);
        }

        // Scope exited, automatically restored to root_ctx
        assert_eq!(current_trace_context(), root_ctx);
    }

    #[test]
    fn test_signpost_interval_duration() {
        let interval = SignpostInterval::begin("test_vfs_scan", 42);
        std::thread::sleep(std::time::Duration::from_millis(2));
        let duration_ns = interval.end();
        assert!(duration_ns >= 1_000_000, "duration should be at least 1ms");
    }
}
