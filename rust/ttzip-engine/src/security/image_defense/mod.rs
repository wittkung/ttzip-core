// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Image 6-Layer Defense-in-Depth Security Subsystem.
//!
//! Enforces deterministic image decoding insulation, memory quota fuses, and recursion limits:
//! 1. **Pixel Bomb Guard** ([`PixelBombGuard`]):
//!    Interception of decompression bombs (single dimension <= 16384, uncompressed memory <= 256MB, expansion ratio <= 250x).
//! 2. **EXIF Safety Guard** ([`ExifSafetyGuard`]):
//!    Interception of EXIF metadata buffer overflows, cyclic IFD loops, and deep recursion (depth <= 4, entries <= 128).
//! 3. **Malformed Chunk Guard** ([`MalformedChunkGuard`]):
//!    Sanitization, self-healing recovery, and escape for truncated/corrupted chunk streams.
//! 4. **ICC Profile Guard** ([`IccProfileGuard`]):
//!    Memory fuse against ICC profile poisoning and multi-dimensional CLUT explosion (size <= 1MB, CLUT memory <= 4.74MB).
//! 5. **Memory Budget Watchdog** ([`MemoryBudgetWatchdog`]):
//!    Deterministic task-level resident memory tracking and fuse circuit breaker (resident memory <= 64MB).
//! 6. **Sensitive Image Buffer** ([`SensitiveImageBuffer`]):
//!    Zero-allocation / zeroize-on-drop pixel memory erasure for confidential and processed image buffers.

pub mod exif_safety;
pub mod icc_profile;
pub mod malformed_chunk;
mod pipeline;
pub mod pixel_bomb;
mod sensitive;
pub mod watchdog;

pub use exif_safety::{ExifInspectionSummary, ExifSafetyGuard};
pub use icc_profile::{IccInspectionSummary, IccProfileGuard};
pub use malformed_chunk::{MalformedChunkGuard, SanitizedChunkReport};
pub use pipeline::ImageSecurityPipeline;
pub use pixel_bomb::{ImageDimensions, PixelBombGuard};
pub use sensitive::SensitiveImageBuffer;
pub use watchdog::{MemoryBudgetWatchdog, MemoryReservation};

// ============================================================================
// Defense Constants & Limits
// ============================================================================

/// Default maximum allowable single dimension for image width or height (16384 px).
pub const DEFAULT_MAX_IMAGE_DIMENSION: u32 = 16384;

/// Default maximum allowable uncompressed raw frame memory buffer (256 MiB).
pub const DEFAULT_MAX_UNCOMPRESSED_MEMORY: usize = 256 * 1024 * 1024;

/// Default maximum allowable ratio of uncompressed bytes to compressed input size (250.0x).
pub const DEFAULT_MAX_IMAGE_EXPANSION_RATIO: f64 = 250.0;

/// Default maximum allowable EXIF IFD recursion / link depth (4 levels).
pub const DEFAULT_MAX_EXIF_RECURSION_DEPTH: usize = 4;

/// Default maximum allowable EXIF tag entries per parsed header (128 entries).
pub const DEFAULT_MAX_EXIF_ENTRIES: usize = 128;

/// Default maximum allowable ICC color profile payload size (1 MiB).
pub const DEFAULT_MAX_ICC_PROFILE_SIZE: usize = 1024 * 1024;

/// Default maximum allowable memory volume for ICC multi-dimensional CLUT tables (4.74 MiB = 4,970,240 bytes).
pub const DEFAULT_MAX_ICC_CLUT_MEMORY: usize = 4_970_240;

/// Default maximum resident memory budget per task (64 MiB).
pub const DEFAULT_MAX_RESIDENT_MEMORY_BUDGET: usize = 64 * 1024 * 1024;

// ============================================================================
// Defense Error Models
// ============================================================================

/// Errors emitted when image security invariants, memory fuses, or format guards are breached.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ImageDefenseError {
    /// Dimension along an axis exceeded the safety ceiling.
    #[error("Image {axis} dimension {dim} exceeds maximum limit of {max_dim} px")]
    DimensionLimitExceeded {
        dim: u32,
        max_dim: u32,
        axis: &'static str,
    },

    /// Pixel bomb decompression explosion detected.
    #[error("Pixel bomb detected: {width}x{height} requires {uncompressed_bytes} bytes (max: {max_bytes} bytes, ratio: {ratio:.2}x > {max_ratio:.2}x)")]
    PixelBombDetected {
        width: u32,
        height: u32,
        uncompressed_bytes: usize,
        max_bytes: usize,
        ratio: f64,
        max_ratio: f64,
    },

    /// EXIF IFD recursion depth exceeded safety limit.
    #[error("EXIF IFD recursion depth exceeded limit ({depth} > {max_depth})")]
    ExifRecursionLimitExceeded { depth: usize, max_depth: usize },

    /// EXIF tag count exceeded maximum allowed limit.
    #[error("EXIF tag count exceeded limit ({count} > {max_count})")]
    ExifTagCountExceeded { count: usize, max_count: usize },

    /// Circular IFD loop detected during EXIF parsing.
    #[error("Circular IFD loop detected at offset 0x{offset:X}")]
    ExifCycleDetected { offset: usize },

    /// Malformed or corrupted EXIF structure encountered.
    #[error("Malformed EXIF metadata: {reason}")]
    ExifMalformed { reason: String },

    /// Malformed chunk structure detected.
    #[error("Malformed chunk '{chunk_type}' at offset {offset}: {reason}")]
    MalformedChunk {
        chunk_type: String,
        offset: usize,
        reason: String,
    },

    /// Premature end of image stream or truncated chunk.
    #[error("Truncated image stream: expected at least {expected_len} bytes, found {actual_len}")]
    TruncatedStream {
        expected_len: usize,
        actual_len: usize,
    },

    /// ICC color profile size exceeded safety threshold.
    #[error("ICC profile size {size} bytes exceeds maximum allowed {max_size} bytes")]
    IccProfileSizeExceeded { size: usize, max_size: usize },

    /// Multi-dimensional CLUT memory allocation exceeded safety budget.
    #[error("ICC profile CLUT memory requirements {bytes} bytes exceed limit of {max_bytes} bytes")]
    IccClutMemoryExceeded { bytes: usize, max_bytes: usize },

    /// Corrupted or invalid ICC profile format.
    #[error("Malformed ICC profile: {reason}")]
    IccMalformed { reason: String },

    /// Memory watchdog budget exceeded by image decoding task.
    #[error("Memory budget watchdog tripped: requested {allocated_bytes} bytes exceeds quota {budget_bytes} bytes")]
    MemoryBudgetExceeded {
        allocated_bytes: usize,
        budget_bytes: usize,
    },

    /// Sensitive buffer zeroize operation failed.
    #[error("Sensitive image buffer zeroize failure: {reason}")]
    ZeroizeFailed { reason: String },

    /// General image defense or decoding error.
    #[error("Image defense validation failed: {0}")]
    GeneralDefenseError(String),
}
