// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Video Media 6-Layer Defense-in-Depth Security Subsystem.
//!
//! Enforces deterministic video container insulation, atom depth ceilings, dimension確界,
//! demuxer loop fuses, subtitle active script neutralization, memory budget tracking, and sensitive zeroization:
//! 1. **Atom Depth Guard** ([`AtomDepthGuard`]):
//!    Interception of nested Atom/Box recursion bombs (depth <= 16), 64-bit largesize arithmetic overflow validation, and out-of-bounds offset circuit breaking.
//! 2. **Video Dimension Guard** ([`VideoDimensionGuard`]):
//!    Strict dimension boundaries (1 <= width <= 8192, 1 <= height <= 8192), uncompressed single-frame memory ceiling (<= 256MB), and divide-by-zero protection.
//! 3. **Demuxer Loop Guard** ([`DemuxerLoopGuard`]):
//!    Circuit breaker against seek infinite loops (iterations <= 1000), consecutive corrupted packets (<= 32), and PTS timestamp monotonicity regressions (backwards tolerance <= 5.0s).
//! 4. **Subtitle Script Sandbox Guard** ([`SubtitleScriptSandboxGuard`]):
//!    Neutralization of external network/file protocols, XSS/script tag stripping, ASS drawing vector quota enforcement (<= 1024 nodes), and path traversal sanitization.
//! 5. **Video Memory Budget Guard** ([`VideoMemoryBudgetGuard`]):
//!    Systemic resident memory tracking enforcing <= 64 MB resident task memory with RAII auto-release reservations.
//! 6. **Sensitive Video Buffer** ([`SensitiveVideoBuffer`]):
//!    Zero-allocation / zeroize-on-drop volatile memory wiping for raw decoded video frames and uncompressed planes.

mod atom_depth;
mod demuxer_loop;
mod dimensions;
mod memory_budget;
mod pipeline;
mod sensitive;
mod subtitle_sandbox;

pub use atom_depth::{AtomDepthGuard, AtomFrame, AtomInspectionSummary, ParsedBoxHeader};
pub use demuxer_loop::{DemuxerLoopGuard, DemuxerLoopTracker};
pub use dimensions::{VideoDimensionGuard, VideoDimensionReport, VideoPixelFormat};
pub use memory_budget::{VideoMemoryBudgetGuard, VideoMemoryReservation};
pub use pipeline::{VideoContainerFormat, VideoSecurityPipeline, VideoSecurityReport};
pub use sensitive::SensitiveVideoBuffer;
pub use subtitle_sandbox::{
    SanitizedSubtitle, SubtitleSanitizeReport, SubtitleScriptSandboxGuard, VideoSubtitleFormat,
};

// ============================================================================
// Defense Constants & Limits
// ============================================================================

/// Default maximum allowable nested Atom / Box hierarchy depth (16 levels).
pub const DEFAULT_MAX_ATOM_DEPTH: usize = 16;

/// Default minimum allowable video dimension along width or height (1 pixel).
pub const DEFAULT_MIN_VIDEO_DIMENSION: u32 = 1;

/// Default maximum allowable video dimension along width or height (8,192 pixels: 8K UHD).
pub const DEFAULT_MAX_VIDEO_DIMENSION: u32 = 8192;

/// Default maximum allowable uncompressed raw frame memory buffer (256 MiB).
pub const DEFAULT_MAX_VIDEO_FRAME_MEMORY: usize = 256 * 1024 * 1024;

/// Default maximum allowable iterations during a single seek resolution attempt (1,000 steps).
pub const DEFAULT_MAX_SEEK_ITERATIONS: usize = 1000;

/// Default maximum allowable consecutive corrupted packet errors before tripping fuse (32 packets).
pub const DEFAULT_MAX_CONSECUTIVE_CORRUPTED_PACKETS: usize = 32;

/// Default maximum allowable cumulative corrupted packet errors before tripping fuse (256 packets).
pub const DEFAULT_MAX_CUMULATIVE_CORRUPTED_PACKETS: usize = 256;

/// Default maximum allowable backwards presentation timestamp (PTS) regression drift in seconds (5.0s).
pub const DEFAULT_MAX_PTS_BACKWARDS_DRIFT_SEC: f64 = 5.0;

/// Default maximum allowable ASS/SSA vector drawing command nodes (1,024 nodes).
pub const DEFAULT_MAX_ASS_DRAWING_NODES: usize = 1024;

/// Default maximum allowable resident memory budget per video task (64 MiB).
pub const DEFAULT_MAX_VIDEO_RESIDENT_MEMORY_BUDGET: usize = 64 * 1024 * 1024;

// ============================================================================
// Defense Error Models
// ============================================================================

/// Errors emitted when video media security invariants, memory fuses, or format limits are breached.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum VideoDefenseError {
    /// Atom / Box nesting depth exceeded the maximum allowed limit.
    #[error("Atom nesting depth exceeded limit ({depth} > {max_depth})")]
    AtomDepthLimitExceeded { depth: usize, max_depth: usize },

    /// 64-bit largesize arithmetic overflow or invalid size representation in container box.
    #[error("Atom '{box_type}' at offset {offset} specifies invalid or overflowing 64-bit largesize {declared_size}")]
    AtomLargesizeOverflow {
        box_type: String,
        offset: u64,
        declared_size: u64,
    },

    /// Atom offset and length exceed total stream or parent container boundary.
    #[error("Atom '{box_type}' at offset {offset} with length {size} extends beyond stream boundary {stream_len}")]
    AtomOutOfBoundsOffset {
        box_type: String,
        offset: u64,
        size: u64,
        stream_len: u64,
    },

    /// Atom length is smaller than required header minimum bytes.
    #[error("Atom '{box_type}' specifies invalid length {size} (minimum required: {min_required})")]
    AtomInvalidSize {
        box_type: String,
        size: u64,
        min_required: u64,
    },

    /// Video dimension along an axis exceeded the safety bounds.
    #[error("Video {axis} dimension {value} is out of bounds (allowed range: {min}..={max} px)")]
    DimensionLimitExceeded {
        axis: &'static str,
        value: u32,
        min: u32,
        max: u32,
    },

    /// Video dimension is zero, causing divide-by-zero or illegal aspect ratio.
    #[error("Video {axis} dimension cannot be zero")]
    InvalidDimensionZero { axis: &'static str },

    /// Video frame memory computation exceeded the uncompressed frame buffer limit.
    #[error("Video frame {width}x{height} requires {estimated_bytes} bytes (maximum allowed: {max_bytes} bytes)")]
    FrameMemoryExceeded {
        width: u32,
        height: u32,
        estimated_bytes: usize,
        max_bytes: usize,
    },

    /// Integer arithmetic overflow during video geometry or stride calculation.
    #[error("Arithmetic overflow encountered during video geometry or stride calculation")]
    DimensionArithmeticOverflow,

    /// Demuxer seek loop exceeded the maximum allowed iteration limit.
    #[error("Demuxer seek loop exceeded iteration limit ({iterations} > {limit})")]
    SeekIterationLimitExceeded { iterations: usize, limit: usize },

    /// Demuxer consecutive packet decode error fuse tripped.
    #[error("Demuxer consecutive corrupted packet error fuse tripped ({consecutive_errors} > {limit})")]
    DemuxerConsecutiveErrorFuse {
        consecutive_errors: usize,
        limit: usize,
    },

    /// Demuxer cumulative packet decode error fuse tripped.
    #[error("Demuxer cumulative corrupted packet error fuse tripped ({cumulative_errors} > {limit})")]
    DemuxerCumulativeErrorFuse {
        cumulative_errors: usize,
        limit: usize,
    },

    /// Demuxer detected a severe non-monotonic PTS backwards regression exceeding tolerance.
    #[error("Demuxer PTS backwards regression detected: from {last_pts:.3}s to {current_pts:.3}s (drop {regression_sec:.3}s > tolerance {max_allowed_sec:.3}s)")]
    PtsMonotonicityRegression {
        last_pts: f64,
        current_pts: f64,
        regression_sec: f64,
        max_allowed_sec: f64,
    },

    /// ASS/SSA subtitle drawing command node count exceeded safety quota.
    #[error("ASS drawing command node count {node_count} exceeds safety limit {limit}")]
    AssDrawingLimitExceeded { node_count: usize, limit: usize },

    /// Subtitle contained an illegal path traversal pattern.
    #[error("Subtitle path traversal or unauthorized local file reference detected: '{path}'")]
    SubtitlePathTraversalDetected { path: String },

    /// Resident memory budget exceeded for video processing task.
    #[error("Video task resident memory budget exceeded: {allocated_bytes} bytes (maximum: {budget_bytes} bytes)")]
    MemoryBudgetExceeded {
        allocated_bytes: usize,
        budget_bytes: usize,
    },

    /// Malformed or corrupted video container header.
    #[error("Malformed video container header: {reason}")]
    MalformedContainerHeader { reason: String },
}
