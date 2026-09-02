// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Audio 6-Layer Defense-in-Depth Security Subsystem.
//!
//! Enforces deterministic audio stream insulation, memory quota fuses, and loop breakers:
//! 1. **Channel and Sample Rate Guard** ([`AudioChannelRateGuard`]):
//!    Interception of invalid channels (0 or > 8) and sample rates (< 8,000 Hz or > 192,000 Hz) to prevent divide-by-zero and heap exhaustion.
//! 2. **Cover Art Quota Guard** ([`CoverArtQuotaGuard`]):
//!    Strict quota limits on embedded album art (single <= 16MB, total <= 32MB, max 4 images) with magic number verification.
//! 3. **Frame Loop Timeout Guard** ([`FrameLoopTimeoutGuard`]):
//!    Circuit breaker against infinite packet loops and corrupted streams (consecutive errors <= 64, cumulative errors <= 256).
//! 4. **ID3 Tag Safety Guard** ([`Id3TagSafetyGuard`]):
//!    Mandatory Syncsafe 7-bit integer validation, tag size quotas (<= 32MB), and memory-safe in-place two-pointer desynchronization.
//! 5. **Memory Budget Guard** ([`AudioMemoryBudgetGuard`]):
//!    Deterministic task-level resident memory tracking and fuse circuit breaker (resident memory <= 64MB).
//! 6. **Sensitive Audio Buffer** ([`SensitiveAudioBuffer`]):
//!    Zero-allocation / zeroize-on-drop audio memory erasure for confidential, cryptographic, and processed PCM streams.

mod channel_rate;
mod cover_art;
mod frame_loop;
mod id3_safety;
mod memory_budget;
mod pipeline;
mod sensitive;

pub use channel_rate::{AudioChannelRateGuard, ChannelRateConfig};
pub use cover_art::{CoverArtFormat, CoverArtInfo, CoverArtQuotaGuard};
pub use frame_loop::{FrameLoopTimeoutGuard, FrameLoopTracker};
pub use id3_safety::{Id3InspectionSummary, Id3TagSafetyGuard};
pub use memory_budget::{AudioMemoryBudgetGuard, AudioMemoryReservation};
pub use pipeline::{AudioSecurityPipeline, AudioSecurityReport};
pub use sensitive::SensitiveAudioBuffer;

// ============================================================================
// Defense Constants & Limits
// ============================================================================

/// Default minimum allowable audio channels (1: Mono).
pub const DEFAULT_MIN_AUDIO_CHANNELS: u16 = 1;

/// Default maximum allowable audio channels (8: 7.1 Surround).
pub const DEFAULT_MAX_AUDIO_CHANNELS: u16 = 8;

/// Default minimum allowable audio sample rate in Hz (8,000 Hz / 8 kHz).
pub const DEFAULT_MIN_SAMPLE_RATE: u32 = 8_000;

/// Default maximum allowable audio sample rate in Hz (192,000 Hz / 192 kHz).
pub const DEFAULT_MAX_SAMPLE_RATE: u32 = 192_000;

/// Default maximum allowable byte size for a single embedded cover art image (16 MiB).
pub const DEFAULT_MAX_SINGLE_COVER_ART_SIZE: usize = 16 * 1024 * 1024;

/// Default maximum allowable cumulative byte size for all embedded cover art images (32 MiB).
pub const DEFAULT_MAX_TOTAL_COVER_ART_SIZE: usize = 32 * 1024 * 1024;

/// Default maximum number of cover art images extracted from an audio container (4 images).
pub const DEFAULT_MAX_COVER_ART_COUNT: usize = 4;

/// Default maximum allowable consecutive frame decode errors before circuit fuse (64 frames).
pub const DEFAULT_MAX_CONSECUTIVE_FRAME_ERRORS: usize = 64;

/// Default maximum allowable cumulative frame decode errors before circuit fuse (256 frames).
pub const DEFAULT_MAX_CUMULATIVE_FRAME_ERRORS: usize = 256;

/// Default maximum allowable ID3v2 tag payload size (32 MiB).
pub const DEFAULT_MAX_ID3_TAG_SIZE: usize = 32 * 1024 * 1024;

/// Default maximum resident memory budget per audio task (64 MiB).
pub const DEFAULT_MAX_AUDIO_RESIDENT_MEMORY_BUDGET: usize = 64 * 1024 * 1024;

// ============================================================================
// Defense Error Models
// ============================================================================

/// Errors emitted when audio security invariants, fuses, or parsing quotas are breached.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AudioDefenseError {
    /// Audio channel count violated safety boundaries.
    #[error("Invalid audio channel count {channels} (allowed range: {min}..={max})")]
    InvalidChannelCount {
        channels: u16,
        min: u16,
        max: u16,
    },

    /// Audio sample rate violated safety boundaries.
    #[error("Invalid audio sample rate {sample_rate} Hz (allowed range: {min}..={max} Hz)")]
    InvalidSampleRate {
        sample_rate: u32,
        min: u32,
        max: u32,
    },

    /// Integer overflow encountered during audio frame size estimation.
    #[error("Audio frame size overflow with {channels} channels and {bits_per_sample} bits/sample")]
    FrameSizeOverflow {
        channels: u16,
        bits_per_sample: u16,
    },

    /// Single embedded cover art image size exceeded safety ceiling.
    #[error("Cover art image size {size} bytes exceeds maximum limit of {max_size} bytes")]
    CoverArtSizeExceeded {
        size: usize,
        max_size: usize,
    },

    /// Cumulative cover art memory quota exceeded safety ceiling.
    #[error("Total cover art size {total_size} bytes exceeds cumulative quota of {max_quota} bytes")]
    TotalCoverArtQuotaExceeded {
        total_size: usize,
        max_quota: usize,
    },

    /// Number of embedded cover art items exceeded allowed limit.
    #[error("Cover art image count {count} exceeds maximum limit of {max_count}")]
    CoverArtCountExceeded {
        count: usize,
        max_count: usize,
    },

    /// Embedded cover art header or magic numbers are corrupted or invalid.
    #[error("Malformed cover art image payload: {reason}")]
    CoverArtMalformed {
        reason: String,
    },

    /// Frame decoder encountered too many consecutive errors, tripping the fuse.
    #[error("Frame loop circuit breaker tripped: {consecutive_errors} consecutive errors (limit: {limit})")]
    FrameLoopConsecutiveErrorFuse {
        consecutive_errors: usize,
        limit: usize,
    },

    /// Frame decoder encountered too many cumulative errors, tripping the fuse.
    #[error("Frame loop circuit breaker tripped: {cumulative_errors} cumulative errors (limit: {limit})")]
    FrameLoopCumulativeErrorFuse {
        cumulative_errors: usize,
        limit: usize,
    },

    /// ID3 tag total size exceeded the configured memory quota.
    #[error("ID3 tag size {size} bytes exceeds maximum quota of {max_size} bytes")]
    Id3TagSizeExceeded {
        size: usize,
        max_size: usize,
    },

    /// Non-syncsafe integer or invalid 7-bit encoding encountered in ID3 header/frame.
    #[error("Invalid ID3 syncsafe integer encoding: {reason}")]
    Id3InvalidSyncsafe {
        reason: String,
    },

    /// Malformed ID3v2 header or tag payload structure.
    #[error("Malformed ID3 tag structure: {reason}")]
    Id3Malformed {
        reason: String,
    },

    /// Audio decoding task resident memory exceeded the watchdog budget.
    #[error("Audio memory budget watchdog tripped: requested {allocated_bytes} bytes exceeds quota {budget_bytes} bytes")]
    MemoryBudgetExceeded {
        allocated_bytes: usize,
        budget_bytes: usize,
    },

    /// Sensitive audio buffer zeroization failure.
    #[error("Sensitive audio buffer zeroize failure: {reason}")]
    ZeroizeFailed {
        reason: String,
    },

    /// General audio defense validation error.
    #[error("Audio defense validation error: {0}")]
    GeneralDefenseError(String),
}
