// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Cross-platform hardware acceleration, portability primitives, and unaligned memory I/O.
//!
//! Provides:
//! - Safe zero-overhead unaligned Little-Endian / Big-Endian read and write primitives.
//! - Dead-Store Elimination (DSE) immune `SecureZeroize` memory wiping.
//! - Runtime CPU feature detection for Apple Silicon NEON / x86 AVX2 dynamic dispatch.
//! - Branch prediction optimization hints (`likely` / `unlikely`).

use std::sync::atomic::{compiler_fence, Ordering};
use std::sync::OnceLock;
use zeroize::Zeroize;

// =============================================================================
// Branch Prediction Optimization Hints
// =============================================================================

/// Informs the compiler optimizer that the boolean condition is expected to evaluate to `true`.
#[inline(always)]
#[must_use]
pub fn likely(b: bool) -> bool {
    if !b {
        cold();
    }
    b
}

/// Informs the compiler optimizer that the boolean condition is expected to evaluate to `false`.
#[inline(always)]
#[must_use]
pub fn unlikely(b: bool) -> bool {
    if b {
        cold();
    }
    b
}

#[cold]
#[inline(always)]
fn cold() {}

// =============================================================================
// Safe & Fast Unaligned Memory Operations (Little-Endian / Big-Endian)
// =============================================================================

/// Reads a 16-bit unsigned integer in little-endian byte order from an unaligned slice.
#[inline(always)]
#[must_use]
pub fn read_unaligned_le16(src: &[u8]) -> u16 {
    assert!(src.len() >= 2, "read_unaligned_le16 out of bounds");
    let mut bytes = [0u8; 2];
    bytes.copy_from_slice(&src[..2]);
    u16::from_le_bytes(bytes)
}

/// Safely reads a 16-bit little-endian integer at the specified byte offset if within bounds.
#[inline(always)]
#[must_use]
pub fn read_unaligned_le16_at(src: &[u8], offset: usize) -> Option<u16> {
    if src.len() >= offset.saturating_add(2) {
        Some(read_unaligned_le16(&src[offset..]))
    } else {
        None
    }
}

/// Reads a 32-bit unsigned integer in little-endian byte order from an unaligned slice.
#[inline(always)]
#[must_use]
pub fn read_unaligned_le32(src: &[u8]) -> u32 {
    assert!(src.len() >= 4, "read_unaligned_le32 out of bounds");
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&src[..4]);
    u32::from_le_bytes(bytes)
}

/// Safely reads a 32-bit little-endian integer at offset if within bounds.
#[inline(always)]
#[must_use]
pub fn read_unaligned_le32_at(src: &[u8], offset: usize) -> Option<u32> {
    if src.len() >= offset.saturating_add(4) {
        Some(read_unaligned_le32(&src[offset..]))
    } else {
        None
    }
}

/// Reads a 64-bit unsigned integer in little-endian byte order from an unaligned slice.
#[inline(always)]
#[must_use]
pub fn read_unaligned_le64(src: &[u8]) -> u64 {
    assert!(src.len() >= 8, "read_unaligned_le64 out of bounds");
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&src[..8]);
    u64::from_le_bytes(bytes)
}

/// Safely reads a 64-bit little-endian integer at offset if within bounds.
#[inline(always)]
#[must_use]
pub fn read_unaligned_le64_at(src: &[u8], offset: usize) -> Option<u64> {
    if src.len() >= offset.saturating_add(8) {
        Some(read_unaligned_le64(&src[offset..]))
    } else {
        None
    }
}

/// Reads a 128-bit unsigned integer in little-endian byte order from an unaligned slice.
#[inline(always)]
#[must_use]
pub fn read_unaligned_le128(src: &[u8]) -> u128 {
    assert!(src.len() >= 16, "read_unaligned_le128 out of bounds");
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&src[..16]);
    u128::from_le_bytes(bytes)
}

/// Reads a 16-bit unsigned integer in big-endian byte order from an unaligned slice.
#[inline(always)]
#[must_use]
pub fn read_unaligned_be16(src: &[u8]) -> u16 {
    assert!(src.len() >= 2, "read_unaligned_be16 out of bounds");
    let mut bytes = [0u8; 2];
    bytes.copy_from_slice(&src[..2]);
    u16::from_be_bytes(bytes)
}

/// Reads a 32-bit unsigned integer in big-endian byte order from an unaligned slice.
#[inline(always)]
#[must_use]
pub fn read_unaligned_be32(src: &[u8]) -> u32 {
    assert!(src.len() >= 4, "read_unaligned_be32 out of bounds");
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&src[..4]);
    u32::from_be_bytes(bytes)
}

/// Reads a 64-bit unsigned integer in big-endian byte order from an unaligned slice.
#[inline(always)]
#[must_use]
pub fn read_unaligned_be64(src: &[u8]) -> u64 {
    assert!(src.len() >= 8, "read_unaligned_be64 out of bounds");
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&src[..8]);
    u64::from_be_bytes(bytes)
}

/// Writes a 16-bit unsigned integer in little-endian byte order into an unaligned slice.
#[inline(always)]
pub fn write_unaligned_le16(dst: &mut [u8], value: u16) {
    assert!(dst.len() >= 2, "write_unaligned_le16 out of bounds");
    dst[..2].copy_from_slice(&value.to_le_bytes());
}

/// Writes a 32-bit unsigned integer in little-endian byte order into an unaligned slice.
#[inline(always)]
pub fn write_unaligned_le32(dst: &mut [u8], value: u32) {
    assert!(dst.len() >= 4, "write_unaligned_le32 out of bounds");
    dst[..4].copy_from_slice(&value.to_le_bytes());
}

/// Writes a 64-bit unsigned integer in little-endian byte order into an unaligned slice.
#[inline(always)]
pub fn write_unaligned_le64(dst: &mut [u8], value: u64) {
    assert!(dst.len() >= 8, "write_unaligned_le64 out of bounds");
    dst[..8].copy_from_slice(&value.to_le_bytes());
}

/// Writes a 128-bit unsigned integer in little-endian byte order into an unaligned slice.
#[inline(always)]
pub fn write_unaligned_le128(dst: &mut [u8], value: u128) {
    assert!(dst.len() >= 16, "write_unaligned_le128 out of bounds");
    dst[..16].copy_from_slice(&value.to_le_bytes());
}

/// Writes a 16-bit unsigned integer in big-endian byte order into an unaligned slice.
#[inline(always)]
pub fn write_unaligned_be16(dst: &mut [u8], value: u16) {
    assert!(dst.len() >= 2, "write_unaligned_be16 out of bounds");
    dst[..2].copy_from_slice(&value.to_be_bytes());
}

/// Writes a 32-bit unsigned integer in big-endian byte order into an unaligned slice.
#[inline(always)]
pub fn write_unaligned_be32(dst: &mut [u8], value: u32) {
    assert!(dst.len() >= 4, "write_unaligned_be32 out of bounds");
    dst[..4].copy_from_slice(&value.to_be_bytes());
}

/// Writes a 64-bit unsigned integer in big-endian byte order into an unaligned slice.
#[inline(always)]
pub fn write_unaligned_be64(dst: &mut [u8], value: u64) {
    assert!(dst.len() >= 8, "write_unaligned_be64 out of bounds");
    dst[..8].copy_from_slice(&value.to_be_bytes());
}

/// Reads a 64-bit little-endian integer from a raw pointer without bounds checks.
///
/// # Safety
/// Caller must ensure `ptr` is valid for reads of at least 8 bytes.
#[inline(always)]
pub unsafe fn read_unaligned_le64_unchecked(ptr: *const u8) -> u64 {
    let raw = std::ptr::read_unaligned(ptr as *const [u8; 8]);
    u64::from_le_bytes(raw)
}

/// Writes a 64-bit little-endian integer to a raw pointer without bounds checks.
///
/// # Safety
/// Caller must ensure `ptr` is valid for writes of at least 8 bytes.
#[inline(always)]
pub unsafe fn write_unaligned_le64_unchecked(ptr: *mut u8, value: u64) {
    std::ptr::write_unaligned(ptr as *mut [u8; 8], value.to_le_bytes());
}

// =============================================================================
// SecureZeroize Memory Wiping (Immune to Dead-Store Elimination)
// =============================================================================

/// Securely wipes memory buffer contents with zero bytes using volatile writes and memory barriers.
pub fn secure_zero_memory(buf: &mut [u8]) {
    for byte in buf.iter_mut() {
        unsafe {
            std::ptr::write_volatile(byte, 0);
        }
    }
    compiler_fence(Ordering::SeqCst);
}

/// Securely wipes any memory structure using volatile operations.
pub fn secure_zero_struct<T: ?Sized>(val: &mut T) {
    let size = std::mem::size_of_val(val);
    let ptr = val as *mut T as *mut u8;
    for i in 0..size {
        unsafe {
            std::ptr::write_volatile(ptr.add(i), 0);
        }
    }
    compiler_fence(Ordering::SeqCst);
}

/// Trait implemented by structures requiring deterministic volatile memory zeroing.
pub trait SecureZeroize {
    /// Zeroes internal sensitive memory buffers.
    fn secure_zeroize(&mut self);
}

impl<T: Zeroize> SecureZeroize for T {
    fn secure_zeroize(&mut self) {
        self.zeroize();
        compiler_fence(Ordering::SeqCst);
    }
}

/// RAII zero-on-drop secure byte buffer for cryptographic keys and secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareSecureBuffer {
    data: Vec<u8>,
}

impl HardwareSecureBuffer {
    /// Creates a new secure buffer from an existing byte slice.
    #[must_use]
    pub fn new(slice: &[u8]) -> Self {
        Self {
            data: slice.to_vec(),
        }
    }

    /// Creates an empty secure buffer with the requested initial capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }

    /// Returns an immutable reference to the inner bytes.
    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// Returns a mutable reference to the inner bytes.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Returns the length of the buffer in bytes.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the buffer contains 0 bytes.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl Drop for HardwareSecureBuffer {
    fn drop(&mut self) {
        secure_zero_memory(&mut self.data);
    }
}

// =============================================================================
// Runtime CPU Feature Detection & Dynamic Dispatch
// =============================================================================

/// Hardware CPU SIMD and cryptographic extension flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuFeature {
    Neon,
    AesNeon,
    ShaNeon,
    Sse42,
    Avx2,
    Avx512,
    ShaX86,
    AesNi,
}

/// Hardware architecture capability profile cached for the current process lifetime.
#[derive(Debug, Clone)]
pub struct HardwareProfile {
    pub has_neon: bool,
    pub has_aes_neon: bool,
    pub has_sha_neon: bool,
    pub has_sse42: bool,
    pub has_avx2: bool,
    pub has_avx512: bool,
    pub has_sha_x86: bool,
    pub has_aes_ni: bool,
}

impl HardwareProfile {
    /// Detects CPU features on the current host machine.
    #[must_use]
    pub fn detect() -> Self {
        #[cfg(target_arch = "aarch64")]
        {
            Self {
                has_neon: true, // Baseline on ARM64 / Apple Silicon
                has_aes_neon: true,
                has_sha_neon: true,
                has_sse42: false,
                has_avx2: false,
                has_avx512: false,
                has_sha_x86: false,
                has_aes_ni: false,
            }
        }
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            Self {
                has_neon: false,
                has_aes_neon: false,
                has_sha_neon: false,
                has_sse42: is_x86_feature_detected!("sse4.2"),
                has_avx2: is_x86_feature_detected!("avx2"),
                has_avx512: is_x86_feature_detected!("avx512f"),
                has_sha_x86: is_x86_feature_detected!("sha"),
                has_aes_ni: is_x86_feature_detected!("aes"),
            }
        }
        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86", target_arch = "x86_64")))]
        {
            Self {
                has_neon: false,
                has_aes_neon: false,
                has_sha_neon: false,
                has_sse42: false,
                has_avx2: false,
                has_avx512: false,
                has_sha_x86: false,
                has_aes_ni: false,
            }
        }
    }

    /// Returns `true` if the specified hardware CPU feature is supported.
    #[must_use]
    pub fn supports(&self, feature: CpuFeature) -> bool {
        match feature {
            CpuFeature::Neon => self.has_neon,
            CpuFeature::AesNeon => self.has_aes_neon,
            CpuFeature::ShaNeon => self.has_sha_neon,
            CpuFeature::Sse42 => self.has_sse42,
            CpuFeature::Avx2 => self.has_avx2,
            CpuFeature::Avx512 => self.has_avx512,
            CpuFeature::ShaX86 => self.has_sha_x86,
            CpuFeature::AesNi => self.has_aes_ni,
        }
    }
}

static HARDWARE_PROFILE: OnceLock<HardwareProfile> = OnceLock::new();

/// Returns a reference to the statically cached runtime hardware profile.
#[must_use]
pub fn detect_hardware_profile() -> &'static HardwareProfile {
    HARDWARE_PROFILE.get_or_init(HardwareProfile::detect)
}

/// Returns `true` if ARM NEON vector instructions are supported on this machine.
#[inline]
#[must_use]
pub fn has_neon() -> bool {
    detect_hardware_profile().has_neon
}

/// Returns `true` if x86 AVX2 vector instructions are supported on this machine.
#[inline]
#[must_use]
pub fn has_avx2() -> bool {
    detect_hardware_profile().has_avx2
}

/// Returns `true` if SSE4.2 instructions are supported on this machine.
#[inline]
#[must_use]
pub fn has_sse42() -> bool {
    detect_hardware_profile().has_sse42
}

/// Returns `true` if dedicated hardware AES acceleration instructions are available.
#[inline]
#[must_use]
pub fn has_aes_hardware() -> bool {
    let profile = detect_hardware_profile();
    profile.has_aes_neon || profile.has_aes_ni
}
