// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Atomic cancellation tokens and cross-thread notification channels.

use crate::types::TTZipStatus;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

const CANCELLED_BIT: u8 = 0x80;
const REASON_MASK: u8 = 0x0F;

/// Cancellation reason conforming to `contracts/ttzip_progress_log_event.json`.
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CancellationReason {
    UserRequested = 0,
    Timeout = 1,
    ResourceExhaustion = 2,
    FatalError = 3,
}

impl CancellationReason {
    pub fn as_str(self) -> &'static str {
        match self {
            CancellationReason::UserRequested => "userRequested",
            CancellationReason::Timeout => "timeout",
            CancellationReason::ResourceExhaustion => "resourceExhaustion",
            CancellationReason::FatalError => "fatalError",
        }
    }

    pub fn from_u8(val: u8) -> Self {
        match val & REASON_MASK {
            1 => CancellationReason::Timeout,
            2 => CancellationReason::ResourceExhaustion,
            3 => CancellationReason::FatalError,
            _ => CancellationReason::UserRequested,
        }
    }
}

/// Thread-safe atomic cancellation token backed by a single `Arc<AtomicU8>`.
#[derive(Debug, Clone)]
pub struct CancellationToken {
    state: Arc<AtomicU8>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationToken {
    /// Creates a new uncancelled token.
    pub fn new() -> Self {
        Self {
            state: Arc::new(AtomicU8::new(0)),
        }
    }

    /// Signals cancellation with the specified reason.
    pub fn cancel(&self, reason: CancellationReason) {
        let val = CANCELLED_BIT | (reason as u8 & REASON_MASK);
        self.state.store(val, Ordering::Release);
    }

    /// Returns true if cancellation has been signalled.
    #[inline(always)]
    pub fn is_cancelled(&self) -> bool {
        self.state.load(Ordering::Acquire) & CANCELLED_BIT != 0
    }

    /// Returns the cancellation reason if cancelled.
    pub fn cancellation_reason(&self) -> Option<CancellationReason> {
        let val = self.state.load(Ordering::Acquire);
        if val & CANCELLED_BIT != 0 {
            Some(CancellationReason::from_u8(val))
        } else {
            None
        }
    }

    /// Helper that returns `Err(TTZipStatus::Cancelled)` if cancelled, or `Ok(())` otherwise.
    #[inline(always)]
    pub fn check(&self) -> Result<(), TTZipStatus> {
        if self.is_cancelled() {
            Err(TTZipStatus::Cancelled)
        } else {
            Ok(())
        }
    }

    /// Creates a child token that shares cancellation state.
    pub fn child_token(&self) -> Self {
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_cancellation_token_basic() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
        assert_eq!(token.check(), Ok(()));
        assert_eq!(token.cancellation_reason(), None);

        token.cancel(CancellationReason::Timeout);
        assert!(token.is_cancelled());
        assert_eq!(token.check(), Err(TTZipStatus::Cancelled));
        assert_eq!(token.cancellation_reason(), Some(CancellationReason::Timeout));
    }

    #[test]
    fn test_cancellation_token_cross_thread() {
        let token = CancellationToken::new();
        let token_clone = token.child_token();

        let handle = thread::spawn(move || {
            while !token_clone.is_cancelled() {
                thread::yield_now();
            }
            token_clone.cancellation_reason()
        });

        thread::sleep(std::time::Duration::from_millis(10));
        token.cancel(CancellationReason::UserRequested);

        let reason = handle.join().expect("join");
        assert_eq!(reason, Some(CancellationReason::UserRequested));
    }
}
