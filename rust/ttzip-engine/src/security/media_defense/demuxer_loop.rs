// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Demuxer Infinite Seek Loop, Packet Corruption Fuse, and PTS Monotonicity Guard.
//!
//! Enforces deterministic runtime circuit breakers against:
//! - Demuxer infinite keyframe bisection search hangs (seek iterations <= 1,000).
//! - Avalanche stream corruption and bit-rot loops (consecutive errors <= 32, cumulative <= 256).
//! - Adversarial presentation timestamp (PTS) regression attacks (backwards drift <= 5.0s).

use super::{
    VideoDefenseError, DEFAULT_MAX_CONSECUTIVE_CORRUPTED_PACKETS,
    DEFAULT_MAX_CUMULATIVE_CORRUPTED_PACKETS, DEFAULT_MAX_PTS_BACKWARDS_DRIFT_SEC,
    DEFAULT_MAX_SEEK_ITERATIONS,
};

/// Configuration parameters for demuxer loop, error thresholds, and timestamp tracking.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DemuxerLoopGuard {
    max_seek_iterations: usize,
    max_consecutive_errors: usize,
    max_cumulative_errors: usize,
    max_pts_drift_sec: f64,
}

impl Default for DemuxerLoopGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl DemuxerLoopGuard {
    /// Creates a guard with default security thresholds (1000 seek steps, 32 consecutive, 256 cumulative, 5.0s PTS drift).
    pub const fn new() -> Self {
        Self {
            max_seek_iterations: DEFAULT_MAX_SEEK_ITERATIONS,
            max_consecutive_errors: DEFAULT_MAX_CONSECUTIVE_CORRUPTED_PACKETS,
            max_cumulative_errors: DEFAULT_MAX_CUMULATIVE_CORRUPTED_PACKETS,
            max_pts_drift_sec: DEFAULT_MAX_PTS_BACKWARDS_DRIFT_SEC,
        }
    }

    /// Creates a guard with custom thresholds.
    pub const fn with_limits(
        max_seek_iterations: usize,
        max_consecutive_errors: usize,
        max_cumulative_errors: usize,
        max_pts_drift_sec: f64,
    ) -> Self {
        Self {
            max_seek_iterations,
            max_consecutive_errors,
            max_cumulative_errors,
            max_pts_drift_sec,
        }
    }

    /// Spawns a stateful runtime tracker for monitoring demuxing and packet streaming.
    pub fn create_tracker(&self) -> DemuxerLoopTracker {
        DemuxerLoopTracker {
            max_seek_iterations: self.max_seek_iterations,
            max_consecutive_errors: self.max_consecutive_errors,
            max_cumulative_errors: self.max_cumulative_errors,
            max_pts_drift_sec: self.max_pts_drift_sec,
            seek_steps: 0,
            consecutive_errors: 0,
            cumulative_errors: 0,
            success_packets: 0,
            last_pts: None,
            last_dts: None,
        }
    }
}

/// Stateful stream watchdog tracking packet decoding, seek operations, and timestamp monotonicity.
#[derive(Debug, Clone, PartialEq)]
pub struct DemuxerLoopTracker {
    max_seek_iterations: usize,
    max_consecutive_errors: usize,
    max_cumulative_errors: usize,
    max_pts_drift_sec: f64,
    seek_steps: usize,
    consecutive_errors: usize,
    cumulative_errors: usize,
    success_packets: usize,
    last_pts: Option<f64>,
    last_dts: Option<f64>,
}

impl DemuxerLoopTracker {
    /// Records a single seek bisection or index traversal iteration. Returns `Err` if seek limit is exceeded.
    pub fn record_seek_step(&mut self) -> Result<(), VideoDefenseError> {
        self.seek_steps = self.seek_steps.saturating_add(1);
        if self.seek_steps > self.max_seek_iterations {
            return Err(VideoDefenseError::SeekIterationLimitExceeded {
                iterations: self.seek_steps,
                limit: self.max_seek_iterations,
            });
        }
        Ok(())
    }

    /// Resets the seek iteration counter upon successful seek target resolution.
    #[inline]
    pub fn reset_seek(&mut self) {
        self.seek_steps = 0;
    }

    /// Explicitly resets the PTS baseline (e.g. after a valid user seek request).
    pub fn reset_timestamp_baseline(&mut self, new_pts: Option<f64>) {
        self.last_pts = new_pts;
        self.last_dts = None;
        self.consecutive_errors = 0;
    }

    /// Records a successfully decoded video/audio packet, validating PTS monotonicity.
    pub fn record_packet_success(
        &mut self,
        pts_sec: f64,
        dts_sec: Option<f64>,
    ) -> Result<(), VideoDefenseError> {
        if let Some(prev_pts) = self.last_pts {
            if pts_sec < prev_pts {
                let regression = prev_pts - pts_sec;
                if regression > self.max_pts_drift_sec {
                    return Err(VideoDefenseError::PtsMonotonicityRegression {
                        last_pts: prev_pts,
                        current_pts: pts_sec,
                        regression_sec: regression,
                        max_allowed_sec: self.max_pts_drift_sec,
                    });
                }
            }
        }

        self.last_pts = Some(pts_sec);
        self.last_dts = dts_sec;
        self.consecutive_errors = 0;
        self.success_packets = self.success_packets.saturating_add(1);

        Ok(())
    }

    /// Records a corrupted packet or demuxer read error. Returns `Err` if safety fuses are tripped.
    pub fn record_packet_error(&mut self) -> Result<(), VideoDefenseError> {
        self.consecutive_errors = self.consecutive_errors.saturating_add(1);
        self.cumulative_errors = self.cumulative_errors.saturating_add(1);

        if self.consecutive_errors > self.max_consecutive_errors {
            return Err(VideoDefenseError::DemuxerConsecutiveErrorFuse {
                consecutive_errors: self.consecutive_errors,
                limit: self.max_consecutive_errors,
            });
        }

        if self.cumulative_errors > self.max_cumulative_errors {
            return Err(VideoDefenseError::DemuxerCumulativeErrorFuse {
                cumulative_errors: self.cumulative_errors,
                limit: self.max_cumulative_errors,
            });
        }

        Ok(())
    }

    /// Returns the number of iterations in the current seek operation.
    #[inline]
    pub const fn seek_steps(&self) -> usize {
        self.seek_steps
    }

    /// Returns the number of consecutive packet errors since the last successful packet.
    #[inline]
    pub const fn consecutive_errors(&self) -> usize {
        self.consecutive_errors
    }

    /// Returns the total cumulative packet errors encountered during stream lifetime.
    #[inline]
    pub const fn cumulative_errors(&self) -> usize {
        self.cumulative_errors
    }

    /// Returns the total successfully decoded packets.
    #[inline]
    pub const fn success_packets(&self) -> usize {
        self.success_packets
    }

    /// Returns the last observed PTS in seconds.
    #[inline]
    pub const fn last_pts(&self) -> Option<f64> {
        self.last_pts
    }

    /// Returns the last observed DTS in seconds.
    #[inline]
    pub const fn last_dts(&self) -> Option<f64> {
        self.last_dts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seek_iteration_breaker() {
        let guard = DemuxerLoopGuard::with_limits(10, 5, 20, 5.0);
        let mut tracker = guard.create_tracker();

        for _ in 0..10 {
            assert!(tracker.record_seek_step().is_ok());
        }
        // 11th iteration exceeds limit of 10
        let err = tracker.record_seek_step().unwrap_err();
        assert_eq!(
            err,
            VideoDefenseError::SeekIterationLimitExceeded {
                iterations: 11,
                limit: 10
            }
        );

        // Reset seek clears steps
        tracker.reset_seek();
        assert_eq!(tracker.seek_steps(), 0);
        assert!(tracker.record_seek_step().is_ok());
    }

    #[test]
    fn test_consecutive_and_cumulative_error_fuses() {
        let guard = DemuxerLoopGuard::with_limits(100, 3, 5, 5.0);
        let mut tracker = guard.create_tracker();

        // 3 consecutive errors: OK
        assert!(tracker.record_packet_error().is_ok());
        assert!(tracker.record_packet_error().is_ok());
        assert!(tracker.record_packet_error().is_ok());

        // 4th consecutive error trips consecutive fuse
        let err = tracker.record_packet_error().unwrap_err();
        assert_eq!(
            err,
            VideoDefenseError::DemuxerConsecutiveErrorFuse {
                consecutive_errors: 4,
                limit: 3
            }
        );

        // Success packet clears consecutive error counter
        assert!(tracker.record_packet_success(1.0, None).is_ok());
        assert_eq!(tracker.consecutive_errors(), 0);
        assert_eq!(tracker.cumulative_errors(), 4);

        // 5th cumulative error: OK (limit is 5)
        assert!(tracker.record_packet_error().is_ok());
        assert_eq!(tracker.cumulative_errors(), 5);

        // 6th cumulative error trips cumulative fuse
        let err_cum = tracker.record_packet_error().unwrap_err();
        assert_eq!(
            err_cum,
            VideoDefenseError::DemuxerCumulativeErrorFuse {
                cumulative_errors: 6,
                limit: 5
            }
        );
    }

    #[test]
    fn test_pts_monotonicity_regression() {
        let guard = DemuxerLoopGuard::with_limits(100, 32, 256, 5.0);
        let mut tracker = guard.create_tracker();

        assert!(tracker.record_packet_success(10.0, None).is_ok());
        assert!(tracker.record_packet_success(12.0, None).is_ok());

        // Minor B-frame PTS reordering within 5.0s tolerance: e.g. from 12.0s to 11.5s (drop 0.5s <= 5.0s) is allowed
        assert!(tracker.record_packet_success(11.5, None).is_ok());

        // Severe malicious regression: from 11.5s back to 1.0s (drop 10.5s > 5.0s)
        let err = tracker.record_packet_success(1.0, None).unwrap_err();
        match err {
            VideoDefenseError::PtsMonotonicityRegression {
                last_pts,
                current_pts,
                regression_sec,
                max_allowed_sec,
            } => {
                assert_eq!(last_pts, 11.5);
                assert_eq!(current_pts, 1.0);
                assert!((regression_sec - 10.5).abs() < 0.001);
                assert_eq!(max_allowed_sec, 5.0);
            }
            _ => panic!("Expected PtsMonotonicityRegression"),
        }
    }
}
