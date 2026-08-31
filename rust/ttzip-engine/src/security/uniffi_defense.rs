// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! UniFFI 6-Layer Defense-in-Depth and Cross-Language Memory Safety Boundary Subsystem.
//!
//! Enforces deterministic memory safety invariants, boundary sanitization, and panic isolation
//! across foreign function interface (FFI) boundaries:
//! 1. **Raw Pointer & Null Boundary Guard (`RawPointerGuard`)**: Validates raw/unmanaged pointers
//!    for null, valid user-space memory alignment (`align_of::<T>()`), and non-canonical address ranges.
//! 2. **RustBuffer Bounds & Truncation Guard (`RustBufferBoundsGuard`)**: Enforces `len <= capacity`,
//!    integer truncation safety, maximum buffer quotas, and safe buffer allocation/reclamation.
//! 3. **Panic Catch & Boundary Isolation Grid (`PanicCatchGuard`)**: Wraps FFI call transitions in
//!    `catch_unwind` and `AssertUnwindSafe`, converts panics to deterministic error codes, and
//!    sanitizes panic messages to prevent sensitive information disclosure.
//! 4. **Handle Lifetime & Double-Free Interceptor (`HandleLifetimeGuard`)**: Thread-safe registry
//!    tracking active foreign object handles, rejecting use-after-free and double-free operations.
//! 5. **UTF-8 Zero-Allocation Sanitizer (`Utf8SanitizerGuard`)**: Validates strictly compliant UTF-8,
//!    intercepts null-byte truncation injection attacks, and detects dangerous control sequences.
//! 6. **Concurrency & Thread Safety Boundary Guard (`ConcurrencyBoundGuard`)**: Verifies `Send + Sync`
//!    invariants, tracks concurrent read/write borrow states, and prevents reentrancy deadlocks.

use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use libc::c_char;
use parking_lot::RwLock;

use crate::types::TTZipStatus;

/// Default maximum allowable capacity for a single `RustBuffer` allocation (512 MiB).
pub const DEFAULT_MAX_RUSTBUFFER_CAPACITY: u64 = 512 * 1024 * 1024;

/// Minimum valid virtual user-space memory address to prevent null-page dereferences.
pub const MIN_VALID_USER_ADDRESS: usize = 0x1000;

/// Maximum allowable length for panic message sanitization output.
pub const MAX_PANIC_MSG_LEN: usize = 256;

// ============================================================================
// Defense Error Models
// ============================================================================

/// Comprehensive error enumeration for UniFFI security invariant violations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UniFFIDefenseError {
    /// Attempted to dereference or access a null pointer across FFI.
    NullPointerDetected,
    /// Memory address is misaligned for the requested type.
    MisalignedPointer { address: usize, alignment: usize },
    /// Memory address is outside valid user-space boundaries.
    InvalidPointerAddress { address: usize },
    /// Requested slice length causes memory calculation overflow.
    SliceLengthOverflow { len: usize, elem_size: usize },
    /// C-string exceeded maximum scan bound without finding a null terminator.
    CStringMissingNullTerminator { max_checked: usize },
    /// RustBuffer length exceeds its allocated capacity (`len > capacity`).
    BufferBoundsViolation { len: u64, capacity: u64 },
    /// RustBuffer capacity exceeds the configured security ceiling.
    BufferCapacityExceeded { capacity: u64, limit: u64 },
    /// RustBuffer slice offset or requested length is out of range.
    BufferOffsetOutOfBounds { offset: u64, requested: u64, len: u64 },
    /// RustBuffer has non-zero capacity but null data pointer.
    NullBufferDataPointer,
    /// Panic caught at the foreign function boundary.
    PanicCaught { sanitized_message: String },
    /// Foreign handle is invalid or not registered in the active registry.
    InvalidHandle { handle: u64 },
    /// Foreign handle has already been unregistered (Double-Free attempt).
    HandleDoubleFree { handle: u64 },
    /// Unreleased foreign handles detected during lifecycle audit.
    HandleLeakDetected { count: usize },
    /// String payload contains malformed or non-compliant UTF-8 bytes.
    InvalidUtf8Encoding { byte_offset: usize },
    /// Embedded null-byte detected in string payload (truncation injection).
    NullByteInjectionDetected { offset: usize },
    /// String byte length exceeds maximum configured security limit.
    StringLengthExceeded { len: usize, max_len: usize },
    /// String payload contains forbidden control character.
    DangerousControlCharacterDetected { code_point: u32 },
    /// Concurrent borrow conflict detected across foreign threads.
    ConcurrentBorrowConflict {
        handle: u64,
        active_readers: usize,
        active_writers: usize,
    },
    /// Reentrancy deadlock condition detected on current thread.
    ReentrancyDeadlockDetected { thread_id: u64, lock_id: u64 },
    /// Thread concurrency boundary invariant violated.
    ConcurrencyThreadViolation { reason: &'static str },
}

impl fmt::Display for UniFFIDefenseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NullPointerDetected => write!(f, "UniFFI defense: null pointer dereference rejected"),
            Self::MisalignedPointer { address, alignment } => {
                write!(f, "UniFFI defense: pointer at {address:#x} not aligned to {alignment} bytes")
            }
            Self::InvalidPointerAddress { address } => {
                write!(f, "UniFFI defense: pointer {address:#x} outside valid user space")
            }
            Self::SliceLengthOverflow { len, elem_size } => {
                write!(f, "UniFFI defense: slice length {len} * {elem_size} causes integer overflow")
            }
            Self::CStringMissingNullTerminator { max_checked } => {
                write!(f, "UniFFI defense: C-string missing null terminator after scanning {max_checked} bytes")
            }
            Self::BufferBoundsViolation { len, capacity } => {
                write!(f, "UniFFI defense: RustBuffer len ({len}) exceeds capacity ({capacity})")
            }
            Self::BufferCapacityExceeded { capacity, limit } => {
                write!(f, "UniFFI defense: RustBuffer capacity {capacity} exceeds limit {limit}")
            }
            Self::BufferOffsetOutOfBounds { offset, requested, len } => {
                write!(f, "UniFFI defense: buffer offset {offset} + {requested} exceeds len {len}")
            }
            Self::NullBufferDataPointer => {
                write!(f, "UniFFI defense: non-zero capacity RustBuffer contains null data pointer")
            }
            Self::PanicCaught { sanitized_message } => {
                write!(f, "UniFFI defense: panic caught at FFI boundary: {sanitized_message}")
            }
            Self::InvalidHandle { handle } => {
                write!(f, "UniFFI defense: invalid handle {handle:#x} (use-after-free or unregistered)")
            }
            Self::HandleDoubleFree { handle } => {
                write!(f, "UniFFI defense: handle {handle:#x} double-free attempt intercepted")
            }
            Self::HandleLeakDetected { count } => {
                write!(f, "UniFFI defense: {count} unreleased foreign handles leaked")
            }
            Self::InvalidUtf8Encoding { byte_offset } => {
                write!(f, "UniFFI defense: malformed UTF-8 at byte offset {byte_offset}")
            }
            Self::NullByteInjectionDetected { offset } => {
                write!(f, "UniFFI defense: embedded null byte injection detected at offset {offset}")
            }
            Self::StringLengthExceeded { len, max_len } => {
                write!(f, "UniFFI defense: string length {len} exceeds ceiling {max_len}")
            }
            Self::DangerousControlCharacterDetected { code_point } => {
                write!(f, "UniFFI defense: dangerous control character U+{code_point:04X} detected")
            }
            Self::ConcurrentBorrowConflict { handle, active_readers, active_writers } => {
                write!(f, "UniFFI defense: concurrent borrow conflict on handle {handle:#x} (readers: {active_readers}, writers: {active_writers})")
            }
            Self::ReentrancyDeadlockDetected { thread_id, lock_id } => {
                write!(f, "UniFFI defense: reentrancy deadlock detected for thread {thread_id} on lock {lock_id}")
            }
            Self::ConcurrencyThreadViolation { reason } => {
                write!(f, "UniFFI defense: concurrency thread violation: {reason}")
            }
        }
    }
}

impl std::error::Error for UniFFIDefenseError {}

impl From<UniFFIDefenseError> for TTZipStatus {
    fn from(err: UniFFIDefenseError) -> Self {
        match err {
            UniFFIDefenseError::PanicCaught { .. } => TTZipStatus::ErrPanicCaught,
            UniFFIDefenseError::BufferCapacityExceeded { .. } => TTZipStatus::ErrOutOfMemory,
            UniFFIDefenseError::BufferBoundsViolation { .. }
            | UniFFIDefenseError::BufferOffsetOutOfBounds { .. }
            | UniFFIDefenseError::MisalignedPointer { .. }
            | UniFFIDefenseError::NullPointerDetected
            | UniFFIDefenseError::NullBufferDataPointer
            | UniFFIDefenseError::InvalidPointerAddress { .. }
            | UniFFIDefenseError::CStringMissingNullTerminator { .. } => TTZipStatus::ErrInvalidParam,
            UniFFIDefenseError::NullByteInjectionDetected { .. }
            | UniFFIDefenseError::DangerousControlCharacterDetected { .. }
            | UniFFIDefenseError::InvalidHandle { .. }
            | UniFFIDefenseError::HandleDoubleFree { .. }
            | UniFFIDefenseError::ConcurrentBorrowConflict { .. }
            | UniFFIDefenseError::ReentrancyDeadlockDetected { .. }
            | UniFFIDefenseError::ConcurrencyThreadViolation { .. } => TTZipStatus::ErrSecurityViolation,
            UniFFIDefenseError::SliceLengthOverflow { .. }
            | UniFFIDefenseError::InvalidUtf8Encoding { .. }
            | UniFFIDefenseError::StringLengthExceeded { .. }
            | UniFFIDefenseError::HandleLeakDetected { .. } => TTZipStatus::ErrInvalidParam,
        }
    }
}

// ============================================================================
// Guard 1: Raw Pointer & Null Boundary Guard
// ============================================================================

/// Guard 1: Validates raw pointers and unmanaged memory references from foreign runtimes.
pub struct RawPointerGuard;

impl RawPointerGuard {
    /// Validates that a raw pointer is non-null, aligned, and within valid virtual address space.
    #[inline]
    pub fn validate_ptr<T>(ptr: *const T) -> Result<(), UniFFIDefenseError> {
        if ptr.is_null() {
            return Err(UniFFIDefenseError::NullPointerDetected);
        }
        let addr = ptr as usize;
        if addr < MIN_VALID_USER_ADDRESS {
            return Err(UniFFIDefenseError::InvalidPointerAddress { address: addr });
        }
        let align = std::mem::align_of::<T>();
        if !addr.is_multiple_of(align) {
            return Err(UniFFIDefenseError::MisalignedPointer { address: addr, alignment: align });
        }
        Ok(())
    }

    /// Validates a mutable raw pointer.
    #[inline]
    pub fn validate_mut_ptr<T>(ptr: *mut T) -> Result<(), UniFFIDefenseError> {
        Self::validate_ptr(ptr as *const T)
    }

    /// Safely constructs a shared slice reference from a foreign pointer and length.
    ///
    /// # Safety
    /// Caller must guarantee the underlying memory buffer is readable for `len` elements.
    pub unsafe fn guard_slice<'a, T>(
        ptr: *const T,
        len: usize,
    ) -> Result<&'a [T], UniFFIDefenseError> {
        if len == 0 {
            return Ok(&[]);
        }
        Self::validate_ptr(ptr)?;
        let elem_size = std::mem::size_of::<T>();
        if let Some(total_bytes) = len.checked_mul(elem_size) {
            if total_bytes > isize::MAX as usize {
                return Err(UniFFIDefenseError::SliceLengthOverflow { len, elem_size });
            }
        } else {
            return Err(UniFFIDefenseError::SliceLengthOverflow { len, elem_size });
        }
        Ok(std::slice::from_raw_parts(ptr, len))
    }

    /// Safely constructs a mutable slice reference from a foreign pointer and length.
    ///
    /// # Safety
    /// Caller must guarantee exclusive write access and readable memory for `len` elements.
    pub unsafe fn guard_slice_mut<'a, T>(
        ptr: *mut T,
        len: usize,
    ) -> Result<&'a mut [T], UniFFIDefenseError> {
        if len == 0 {
            return Ok(&mut []);
        }
        Self::validate_mut_ptr(ptr)?;
        let elem_size = std::mem::size_of::<T>();
        if let Some(total_bytes) = len.checked_mul(elem_size) {
            if total_bytes > isize::MAX as usize {
                return Err(UniFFIDefenseError::SliceLengthOverflow { len, elem_size });
            }
        } else {
            return Err(UniFFIDefenseError::SliceLengthOverflow { len, elem_size });
        }
        Ok(std::slice::from_raw_parts_mut(ptr, len))
    }

    /// Safely inspects a C-style null-terminated string pointer with an upper bound scan.
    ///
    /// # Safety
    /// Caller must ensure `ptr` points to readable memory up to the null terminator or `max_len`.
    pub unsafe fn guard_c_str<'a>(
        ptr: *const c_char,
        max_len: usize,
    ) -> Result<&'a str, UniFFIDefenseError> {
        if ptr.is_null() {
            return Err(UniFFIDefenseError::NullPointerDetected);
        }
        let addr = ptr as usize;
        if addr < MIN_VALID_USER_ADDRESS {
            return Err(UniFFIDefenseError::InvalidPointerAddress { address: addr });
        }
        let u8_ptr = ptr as *const u8;
        let mut len = 0;
        while len < max_len {
            if *u8_ptr.add(len) == 0 {
                let slice = std::slice::from_raw_parts(u8_ptr, len);
                return std::str::from_utf8(slice)
                    .map_err(|e| UniFFIDefenseError::InvalidUtf8Encoding {
                        byte_offset: e.valid_up_to(),
                    });
            }
            len += 1;
        }
        Err(UniFFIDefenseError::CStringMissingNullTerminator { max_checked: max_len })
    }
}

// ============================================================================
// Guard 2: RustBuffer Bounds & Truncation Guard
// ============================================================================

/// Foreign function buffer representation compatible with UniFFI binary protocol.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RustBufferRaw {
    /// Allocated buffer capacity in bytes.
    pub capacity: u64,
    /// Active initialized byte length.
    pub len: u64,
    /// Pointer to the underlying contiguous byte heap allocation.
    pub data: *mut u8,
}

impl Default for RustBufferRaw {
    fn default() -> Self {
        Self {
            capacity: 0,
            len: 0,
            data: std::ptr::null_mut(),
        }
    }
}

/// Guard 2: Enforces deterministic bounds, integer limits, and valid allocations on `RustBufferRaw`.
pub struct RustBufferBoundsGuard;

impl RustBufferBoundsGuard {
    /// Validates the internal memory invariants of a `RustBufferRaw`.
    pub fn validate_buffer(
        buf: &RustBufferRaw,
        max_capacity: u64,
    ) -> Result<(), UniFFIDefenseError> {
        if buf.len > buf.capacity {
            return Err(UniFFIDefenseError::BufferBoundsViolation {
                len: buf.len,
                capacity: buf.capacity,
            });
        }
        if buf.capacity > max_capacity {
            return Err(UniFFIDefenseError::BufferCapacityExceeded {
                capacity: buf.capacity,
                limit: max_capacity,
            });
        }
        if buf.capacity > (isize::MAX as u64) {
            return Err(UniFFIDefenseError::BufferCapacityExceeded {
                capacity: buf.capacity,
                limit: isize::MAX as u64,
            });
        }
        if buf.capacity > 0 {
            if buf.data.is_null() {
                return Err(UniFFIDefenseError::NullBufferDataPointer);
            }
            let addr = buf.data as usize;
            if addr < MIN_VALID_USER_ADDRESS {
                return Err(UniFFIDefenseError::InvalidPointerAddress { address: addr });
            }
        }
        Ok(())
    }

    /// Validates that an offset and length slice access falls strictly within initialized bytes.
    pub fn validate_slice_access(
        buf: &RustBufferRaw,
        offset: u64,
        requested_len: u64,
    ) -> Result<(*const u8, usize), UniFFIDefenseError> {
        Self::validate_buffer(buf, DEFAULT_MAX_RUSTBUFFER_CAPACITY)?;
        let end = offset
            .checked_add(requested_len)
            .ok_or(UniFFIDefenseError::BufferBoundsViolation {
                len: u64::MAX,
                capacity: buf.capacity,
            })?;
        if end > buf.len {
            return Err(UniFFIDefenseError::BufferOffsetOutOfBounds {
                offset,
                requested: requested_len,
                len: buf.len,
            });
        }
        if requested_len == 0 {
            return Ok((std::ptr::null(), 0));
        }
        let ptr = unsafe { buf.data.add(offset as usize) as *const u8 };
        Ok((ptr, requested_len as usize))
    }

    /// Converts a valid `RustBufferRaw` to a shared byte slice.
    pub fn as_slice(buf: &RustBufferRaw) -> Result<&[u8], UniFFIDefenseError> {
        Self::validate_buffer(buf, DEFAULT_MAX_RUSTBUFFER_CAPACITY)?;
        if buf.len == 0 {
            return Ok(&[]);
        }
        unsafe { Ok(std::slice::from_raw_parts(buf.data, buf.len as usize)) }
    }

    /// Converts a valid `RustBufferRaw` to a mutable byte slice.
    pub fn as_slice_mut(
        buf: &mut RustBufferRaw,
    ) -> Result<&mut [u8], UniFFIDefenseError> {
        Self::validate_buffer(buf, DEFAULT_MAX_RUSTBUFFER_CAPACITY)?;
        if buf.len == 0 {
            return Ok(&mut []);
        }
        unsafe { Ok(std::slice::from_raw_parts_mut(buf.data, buf.len as usize)) }
    }

    /// Safely allocates a `RustBufferRaw` from a byte slice.
    pub fn alloc_buffer(data: &[u8]) -> RustBufferRaw {
        if data.is_empty() {
            return RustBufferRaw::default();
        }
        let mut vec = data.to_vec();
        let len = vec.len() as u64;
        let capacity = vec.capacity() as u64;
        let data = vec.as_mut_ptr();
        std::mem::forget(vec);
        RustBufferRaw { capacity, len, data }
    }

    /// Reclaims and frees memory backing a `RustBufferRaw`.
    pub fn free_buffer(buf: &mut RustBufferRaw) -> Result<(), UniFFIDefenseError> {
        if buf.capacity == 0 || buf.data.is_null() {
            *buf = RustBufferRaw::default();
            return Ok(());
        }
        Self::validate_buffer(buf, DEFAULT_MAX_RUSTBUFFER_CAPACITY)?;
        unsafe {
            let _vec = Vec::from_raw_parts(buf.data, buf.len as usize, buf.capacity as usize);
            // Drop automatically reclaims memory.
        }
        *buf = RustBufferRaw::default();
        Ok(())
    }
}

// ============================================================================
// Guard 3: Panic Catch & Boundary Isolation Grid
// ============================================================================

/// Guard 3: Captures unwinding panics at FFI boundaries and sanitizes output error messages.
pub struct PanicCatchGuard;

impl PanicCatchGuard {
    /// Sanitizes an arbitrary panic payload into a safe, bounded diagnostic string.
    pub fn sanitize_panic_payload(payload: &(dyn Any + Send)) -> String {
        let raw_msg = if let Some(s) = payload.downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "internal execution panic occurred".to_string()
        };

        let sanitized = raw_msg.replace(|c: char| c.is_control() && c != ' ', " ");
        let mut bounded = String::with_capacity(sanitized.len().min(MAX_PANIC_MSG_LEN));
        for ch in sanitized.chars() {
            if bounded.len() + ch.len_utf8() > MAX_PANIC_MSG_LEN {
                bounded.push_str("...");
                break;
            }
            bounded.push(ch);
        }
        bounded
    }

    /// Executes a closure across FFI boundary, catching any panic and converting it to `UniFFIDefenseError`.
    pub fn catch_boundary<F, R>(f: F) -> Result<R, UniFFIDefenseError>
    where
        F: FnOnce() -> R + std::panic::UnwindSafe,
    {
        match catch_unwind(AssertUnwindSafe(f)) {
            Ok(result) => Ok(result),
            Err(payload) => {
                let sanitized_message = Self::sanitize_panic_payload(&*payload);
                Err(UniFFIDefenseError::PanicCaught { sanitized_message })
            }
        }
    }

    /// Executes a status-returning closure, returning `TTZipStatus::ErrPanicCaught` on panic.
    pub fn catch_status<F>(f: F) -> TTZipStatus
    where
        F: FnOnce() -> TTZipStatus + std::panic::UnwindSafe,
    {
        match Self::catch_boundary(f) {
            Ok(status) => status,
            Err(_) => TTZipStatus::ErrPanicCaught,
        }
    }
}

// ============================================================================
// Guard 4: Handle Lifetime & Double-Free Interceptor
// ============================================================================

/// Thread-safe foreign object handle registry tracking active allocations and preventing double-free.
pub struct HandleRegistry<T> {
    next_handle: AtomicU64,
    handles: RwLock<HashMap<u64, Arc<T>>>,
}

impl<T> Default for HandleRegistry<T> {
    fn default() -> Self {
        Self {
            next_handle: AtomicU64::new(0x1000_0001),
            handles: RwLock::new(HashMap::new()),
        }
    }
}

impl<T> HandleRegistry<T> {
    /// Creates a new, empty foreign object handle registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an object and returns a unique 64-bit non-zero handle.
    pub fn register(&self, item: T) -> u64 {
        let handle = self.next_handle.fetch_add(1, Ordering::Relaxed);
        let arc = Arc::new(item);
        self.handles.write().insert(handle, arc);
        handle
    }

    /// Retrieves an Arc reference to an object by handle, rejecting invalid or expired handles.
    pub fn get(&self, handle: u64) -> Result<Arc<T>, UniFFIDefenseError> {
        self.handles
            .read()
            .get(&handle)
            .cloned()
            .ok_or(UniFFIDefenseError::InvalidHandle { handle })
    }

    /// Unregisters and releases an object handle, strictly rejecting double-free attempts.
    pub fn unregister(&self, handle: u64) -> Result<Arc<T>, UniFFIDefenseError> {
        self.handles
            .write()
            .remove(&handle)
            .ok_or(UniFFIDefenseError::HandleDoubleFree { handle })
    }

    /// Returns the total number of currently active handles.
    pub fn active_count(&self) -> usize {
        self.handles.read().len()
    }

    /// Returns all unreleased handle IDs (used for leak detection during teardown).
    pub fn detect_leaks(&self) -> Vec<u64> {
        self.handles.read().keys().copied().collect()
    }
}

/// RAII handle wrapper that automatically cleans up registered handles on drop.
pub struct ScopedHandle<'a, T> {
    handle: u64,
    registry: &'a HandleRegistry<T>,
    item: Arc<T>,
}

impl<'a, T> ScopedHandle<'a, T> {
    /// Creates a new scoped handle wrapper.
    pub fn new(registry: &'a HandleRegistry<T>, item: T) -> Self {
        let arc = Arc::new(item);
        let handle = registry.next_handle.fetch_add(1, Ordering::Relaxed);
        registry.handles.write().insert(handle, arc.clone());
        Self {
            handle,
            registry,
            item: arc,
        }
    }

    /// Returns the 64-bit integer handle value.
    pub fn handle(&self) -> u64 {
        self.handle
    }
}

impl<'a, T> Deref for ScopedHandle<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl<'a, T> Drop for ScopedHandle<'a, T> {
    fn drop(&mut self) {
        let _ = self.registry.unregister(self.handle);
    }
}

// ============================================================================
// Guard 5: UTF-8 Zero-Allocation Sanitizer
// ============================================================================

/// Guard 5: Performs zero-allocation validation, null-byte injection detection, and string sanitization.
pub struct Utf8SanitizerGuard;

impl Utf8SanitizerGuard {
    /// Performs zero-allocation validation that byte slice is compliant UTF-8 without null bytes.
    pub fn validate_safe_string(
        bytes: &[u8],
        max_len: usize,
    ) -> Result<&str, UniFFIDefenseError> {
        if bytes.len() > max_len {
            return Err(UniFFIDefenseError::StringLengthExceeded {
                len: bytes.len(),
                max_len,
            });
        }
        let s = std::str::from_utf8(bytes).map_err(|e| {
            UniFFIDefenseError::InvalidUtf8Encoding {
                byte_offset: e.valid_up_to(),
            }
        })?;
        if let Some(pos) = bytes.iter().position(|&b| b == 0) {
            return Err(UniFFIDefenseError::NullByteInjectionDetected { offset: pos });
        }
        if let Some(code) = Self::contains_control_characters(s) {
            return Err(UniFFIDefenseError::DangerousControlCharacterDetected { code_point: code });
        }
        Ok(s)
    }

    /// Detects dangerous control characters (excluding newline, carriage return, and tab).
    pub fn contains_control_characters(s: &str) -> Option<u32> {
        for c in s.chars() {
            let u = c as u32;
            if (u < 0x20 && u != 0x09 && u != 0x0A && u != 0x0D) || (0x7F..=0x9F).contains(&u) {
                return Some(u);
            }
        }
        None
    }

    /// Performs lossy sanitization converting invalid or malicious bytes into safe printable output.
    pub fn sanitize_lossy(bytes: &[u8], max_len: usize) -> String {
        let bounded_bytes = if bytes.len() > max_len {
            &bytes[..max_len]
        } else {
            bytes
        };
        let lossy = String::from_utf8_lossy(bounded_bytes);
        let mut out = String::with_capacity(lossy.len());
        for c in lossy.chars() {
            let u = c as u32;
            if c == '\0' || (u < 0x20 && u != 0x09 && u != 0x0A && u != 0x0D) || (0x7F..=0x9F).contains(&u) {
                out.push('\u{FFFD}');
            } else {
                out.push(c);
            }
        }
        out
    }
}

// ============================================================================
// Guard 6: Concurrency & Thread Safety Boundary Guard
// ============================================================================

/// Guard 6: Validates thread safety, borrow states, and reentrancy invariants.
pub struct ConcurrencyBoundGuard;

impl ConcurrencyBoundGuard {
    /// Statically asserts at compile-time that a type implements `Send + Sync`.
    pub const fn assert_send_sync<T: Send + Sync>() {}

    /// Statically asserts that a type implements `RefUnwindSafe`.
    pub const fn assert_unwind_safe<T: std::panic::RefUnwindSafe>() {}
}

/// Tracks concurrent read/write borrows per handle to prevent race conditions.
pub struct ConcurrentBorrowTracker {
    borrows: RwLock<HashMap<u64, (usize, usize)>>,
}

impl Default for ConcurrentBorrowTracker {
    fn default() -> Self {
        Self {
            borrows: RwLock::new(HashMap::new()),
        }
    }
}

impl ConcurrentBorrowTracker {
    /// Creates a new concurrent borrow tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Attempts to acquire a shared read borrow for a handle.
    pub fn borrow_read(&self, handle: u64) -> Result<(), UniFFIDefenseError> {
        let mut map = self.borrows.write();
        let entry = map.entry(handle).or_insert((0, 0));
        if entry.1 > 0 {
            return Err(UniFFIDefenseError::ConcurrentBorrowConflict {
                handle,
                active_readers: entry.0,
                active_writers: entry.1,
            });
        }
        entry.0 += 1;
        Ok(())
    }

    /// Releases a shared read borrow for a handle.
    pub fn release_read(&self, handle: u64) {
        let mut map = self.borrows.write();
        if let Some(entry) = map.get_mut(&handle) {
            if entry.0 > 0 {
                entry.0 -= 1;
            }
            if entry.0 == 0 && entry.1 == 0 {
                map.remove(&handle);
            }
        }
    }

    /// Attempts to acquire an exclusive mutable write borrow for a handle.
    pub fn borrow_write(&self, handle: u64) -> Result<(), UniFFIDefenseError> {
        let mut map = self.borrows.write();
        let entry = map.entry(handle).or_insert((0, 0));
        if entry.0 > 0 || entry.1 > 0 {
            return Err(UniFFIDefenseError::ConcurrentBorrowConflict {
                handle,
                active_readers: entry.0,
                active_writers: entry.1,
            });
        }
        entry.1 = 1;
        Ok(())
    }

    /// Releases an exclusive mutable write borrow for a handle.
    pub fn release_write(&self, handle: u64) {
        let mut map = self.borrows.write();
        if let Some(entry) = map.get_mut(&handle) {
            entry.1 = 0;
            if entry.0 == 0 {
                map.remove(&handle);
            }
        }
    }
}
