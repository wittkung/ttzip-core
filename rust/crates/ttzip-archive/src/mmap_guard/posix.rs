// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! POSIX Memory-Mapped Slice Protection & Bounds Checking.
//!
//! Restricts `mmap` strictly to read-only random-access metadata index parsing.
//! Payload streaming is handled by direct system calls (`pread` / `read_exact_at`)
//! with kernel readahead hints, eliminating `SIGBUS` hazards and avoiding any
//! non-local signal jumps (`siglongjmp`) that bypass Rust RAII destructors.

use std::io::Result;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

const SLOT_FREE: u8 = 0;
const SLOT_INITIALIZING: u8 = 1;
const SLOT_ACTIVE: u8 = 2;
const MAX_GUARDED_REGIONS: usize = 256;

struct AtomicGuardedSlot {
    start: AtomicUsize,
    end: AtomicUsize,
    state: AtomicU8,
}

impl AtomicGuardedSlot {
    const fn new() -> Self {
        Self {
            start: AtomicUsize::new(0),
            end: AtomicUsize::new(0),
            state: AtomicU8::new(SLOT_FREE),
        }
    }
}

static GUARDED_REGIONS: [AtomicGuardedSlot; MAX_GUARDED_REGIONS] =
    [const { AtomicGuardedSlot::new() }; MAX_GUARDED_REGIONS];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuardToken(pub usize);

pub fn register_guarded_range(start: usize, len: usize) -> Option<GuardToken> {
    if len == 0 {
        return None;
    }
    let end = start.checked_add(len)?;

    for (idx, slot) in GUARDED_REGIONS.iter().enumerate() {
        if slot.state.load(Ordering::Relaxed) == SLOT_FREE
            && slot
                .state
                .compare_exchange(SLOT_FREE, SLOT_INITIALIZING, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
        {
            slot.start.store(start, Ordering::Relaxed);
            slot.end.store(end, Ordering::Relaxed);
            slot.state.store(SLOT_ACTIVE, Ordering::Release);
            return Some(GuardToken(idx));
        }
    }
    None
}

pub fn unregister_guarded_range(token: GuardToken) {
    if token.0 < MAX_GUARDED_REGIONS {
        let slot = &GUARDED_REGIONS[token.0];
        slot.state.store(SLOT_FREE, Ordering::Release);
        slot.start.store(0, Ordering::Relaxed);
        slot.end.store(0, Ordering::Relaxed);
    }
}

#[inline]
pub fn is_guarded_address(addr: usize) -> bool {
    for slot in GUARDED_REGIONS.iter() {
        if slot.state.load(Ordering::Acquire) == SLOT_ACTIVE {
            let start = slot.start.load(Ordering::Relaxed);
            let end = slot.end.load(Ordering::Relaxed);
            if addr >= start && addr < end {
                return true;
            }
        }
    }
    false
}

#[inline]
pub fn ensure_signal_handlers_installed() {
    // Dangerous signal jumps that bypass Rust unwinding have been permanently eliminated.
}

#[inline]
pub fn with_mmap_guard<F, R>(f: F) -> Result<R>
where
    F: FnOnce() -> Result<R>,
{
    f()
}

pub fn safe_copy_bytes(src: &[u8], dst: &mut [u8]) -> Result<usize> {
    let to_copy = src.len().min(dst.len());
    if to_copy == 0 {
        return Ok(0);
    }
    dst[..to_copy].copy_from_slice(&src[..to_copy]);
    Ok(to_copy)
}

#[cfg(all(unix, test))]
mod tests {
    use super::*;

    #[test]
    fn test_normal_read_with_guard() {
        let data = vec![1u8, 2, 3, 4, 5];
        let token = register_guarded_range(data.as_ptr() as usize, data.len()).expect("register");
        let mut dst = [0u8; 5];
        let res = safe_copy_bytes(&data, &mut dst);
        unregister_guarded_range(token);
        assert!(res.is_ok());
        assert_eq!(&dst, &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_range_registration() {
        let dummy = [0u8; 64];
        let token = register_guarded_range(dummy.as_ptr() as usize, dummy.len()).expect("register");
        assert!(is_guarded_address(dummy.as_ptr() as usize));
        assert!(is_guarded_address(dummy.as_ptr() as usize + 32));
        assert!(!is_guarded_address(dummy.as_ptr() as usize + 64));
        unregister_guarded_range(token);
        assert!(!is_guarded_address(dummy.as_ptr() as usize));
    }
}
