// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Audio Frame Loop Timeout and Corrupted Stream Circuit Breaker.
//!
//! Intercepts infinite frame repetition attacks, unrecoverable stream corruption loops,
//! and spin-lock hangs during packet decoding by tracking consecutive and cumulative errors.

use super::{
    AudioDefenseError, DEFAULT_MAX_CONSECUTIVE_FRAME_ERRORS, DEFAULT_MAX_CUMULATIVE_FRAME_ERRORS,
};

/// Circuit breaker guard configuring thresholds for frame decoding timeouts and error limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameLoopTimeoutGuard {
    max_consecutive_errors: usize,
    max_cumulative_errors: usize,
}

impl Default for FrameLoopTimeoutGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameLoopTimeoutGuard {
    /// Creates a guard with default security thresholds (64 consecutive, 256 cumulative).
    pub const fn new() -> Self {
        Self {
            max_consecutive_errors: DEFAULT_MAX_CONSECUTIVE_FRAME_ERRORS,
            max_cumulative_errors: DEFAULT_MAX_CUMULATIVE_FRAME_ERRORS,
        }
    }

    /// Creates a guard with custom error limits.
    pub const fn with_limits(max_consecutive: usize, max_cumulative: usize) -> Self {
        Self {
            max_consecutive_errors: max_consecutive,
            max_cumulative_errors: max_cumulative,
        }
    }

    /// Spawns a stateful tracker instance for monitoring an active audio decoding stream.
    pub fn create_tracker(&self) -> FrameLoopTracker {
        FrameLoopTracker {
            max_consecutive: self.max_consecutive_errors,
            max_cumulative: self.max_cumulative_errors,
            consecutive_errors: 0,
            cumulative_errors: 0,
            success_frames: 0,
        }
    }
}

/// Stateful stream watchdog tracking decode progress and tripping fuses upon excessive failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameLoopTracker {
    max_consecutive: usize,
    max_cumulative: usize,
    consecutive_errors: usize,
    cumulative_errors: usize,
    success_frames: usize,
}

impl FrameLoopTracker {
    /// Records a successfully decoded audio frame or packet, resetting the consecutive error counter.
    #[inline]
    pub fn record_success(&mut self) {
        self.consecutive_errors = 0;
        self.success_frames = self.success_frames.saturating_add(1);
    }

    /// Records a decode error or corrupted frame. Returns `Err` if any safety fuse is tripped.
    pub fn record_error(&mut self) -> Result<(), AudioDefenseError> {
        self.consecutive_errors = self.consecutive_errors.saturating_add(1);
        self.cumulative_errors = self.cumulative_errors.saturating_add(1);

        if self.consecutive_errors > self.max_consecutive {
            return Err(AudioDefenseError::FrameLoopConsecutiveErrorFuse {
                consecutive_errors: self.consecutive_errors,
                limit: self.max_consecutive,
            });
        }

        if self.cumulative_errors > self.max_cumulative {
            return Err(AudioDefenseError::FrameLoopCumulativeErrorFuse {
                cumulative_errors: self.cumulative_errors,
                limit: self.max_cumulative,
            });
        }

        Ok(())
    }

    /// Returns the number of consecutive decode errors encountered since the last successful frame.
    #[inline]
    pub const fn consecutive_errors(&self) -> usize {
        self.consecutive_errors
    }

    /// Returns the total cumulative decode errors encountered during the stream lifetime.
    #[inline]
    pub const fn cumulative_errors(&self) -> usize {
        self.cumulative_errors
    }

    /// Returns the count of successfully decoded frames.
    #[inline]
    pub const fn success_frames(&self) -> usize {
        self.success_frames
    }

    /// Checks if the tracker has not tripped any error threshold.
    #[inline]
    pub const fn is_healthy(&self) -> bool {
        self.consecutive_errors <= self.max_consecutive
            && self.cumulative_errors <= self.max_cumulative
    }

    /// Resets all counters to initial zero state.
    pub fn reset(&mut self) {
        self.consecutive_errors = 0;
        self.cumulative_errors = 0;
        self.success_frames = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consecutive_error_fuse_trigger() {
        let guard = FrameLoopTimeoutGuard::with_limits(5, 20);
        let mut tracker = guard.create_tracker();

        for _ in 0..5 {
            assert!(tracker.record_error().is_ok());
        }

        // 6th consecutive error exceeds limit of 5
        let err = tracker.record_error().unwrap_err();
        assert_eq!(
            err,
            AudioDefenseError::FrameLoopConsecutiveErrorFuse {
                consecutive_errors: 6,
                limit: 5
            }
        );
    }

    #[test]
    fn test_consecutive_error_reset_on_success() {
        let guard = FrameLoopTimeoutGuard::with_limits(5, 20);
        let mut tracker = guard.create_tracker();

        for _ in 0..4 {
            assert!(tracker.record_error().is_ok());
        }
        assert_eq!(tracker.consecutive_errors(), 4);

        // Success resets consecutive errors
        tracker.record_success();
        assert_eq!(tracker.consecutive_errors(), 0);
        assert_eq!(tracker.success_frames(), 1);

        // Can tolerate 4 more errors without tripping consecutive fuse
        for _ in 0..4 {
            assert!(tracker.record_error().is_ok());
        }
        assert_eq!(tracker.consecutive_errors(), 4);
    }

    #[test]
    fn test_cumulative_error_fuse_trigger() {
        let guard = FrameLoopTimeoutGuard::with_limits(5, 10);
        let mut tracker = guard.create_tracker();

        // 3 errors then success (3 errors cumulative)
        for _ in 0..3 {
            assert!(tracker.record_error().is_ok());
        }
        tracker.record_success();

        // 3 errors then success (6 errors cumulative)
        for _ in 0..3 {
            assert!(tracker.record_error().is_ok());
        }
        tracker.record_success();

        // 4 errors (10 errors cumulative)
        for _ in 0..4 {
            assert!(tracker.record_error().is_ok());
        }
        assert_eq!(tracker.cumulative_errors(), 10);

        // 11th error exceeds cumulative limit of 10
        let err = tracker.record_error().unwrap_err();
        assert_eq!(
            err,
            AudioDefenseError::FrameLoopCumulativeErrorFuse {
                cumulative_errors: 11,
                limit: 10
            }
        );
    }

    #[test]
    fn test_tracker_health_and_reset() {
        let guard = FrameLoopTimeoutGuard::with_limits(5, 10);
        let mut tracker = guard.create_tracker();
        assert!(tracker.is_healthy());

        tracker.record_success();
        assert_eq!(tracker.success_frames(), 1);

        tracker.reset();
        assert_eq!(tracker.success_frames(), 0);
        assert_eq!(tracker.consecutive_errors(), 0);
        assert_eq!(tracker.cumulative_errors(), 0);
    }
}
