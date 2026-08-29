// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Thermal Throttle Governor & Active Heat Dissipation State Machine.
//!
//! Provides automated 70s active workload tracking with 10s cool-down sleep cycles
//! to prevent CPU thermal throttling and frequency scaling drift during heavy A/B benchmarks.

use std::time::{Duration, Instant};

/// Standard sustained active workload threshold (70 seconds in microseconds).
pub const ACTIVE_PERIOD_MICROS: u64 = 70_000_000;

/// Standard active cool-down sleep duration (10 seconds).
pub const COOL_PERIOD_SECS: u64 = 10;

/// Default cooldown duration as `Duration`.
pub const DEFAULT_COOL_PERIOD: Duration = Duration::from_secs(COOL_PERIOD_SECS);

/// State machine governing CPU thermal dissipation during continuous benchmark passes.
#[derive(Debug, Clone)]
pub struct ThermalThrottleGovernor {
    /// Active workload duration limit before cooldown (microseconds).
    pub active_period_micros: u64,
    /// Cooldown sleep duration when threshold is reached.
    pub cool_period: Duration,
    /// Cumulative active computation time in the current cycle (microseconds).
    pub accumulated_active_micros: u64,
    /// Total cumulative active computation time across all cycles (microseconds).
    pub total_active_micros: u64,
    /// Total number of cooldown periods triggered.
    pub total_cooldowns_triggered: usize,
    /// Timestamp when the current active pass began (if tracking passes).
    pub current_pass_start: Option<Instant>,
    /// Whether to automatically execute `std::thread::sleep` when cooling is triggered.
    pub auto_sleep: bool,
}

impl Default for ThermalThrottleGovernor {
    fn default() -> Self {
        Self::new()
    }
}

impl ThermalThrottleGovernor {
    /// Creates a default governor configured with standard 70s active / 10s cooling cycles.
    pub fn new() -> Self {
        Self {
            active_period_micros: ACTIVE_PERIOD_MICROS,
            cool_period: DEFAULT_COOL_PERIOD,
            accumulated_active_micros: 0,
            total_active_micros: 0,
            total_cooldowns_triggered: 0,
            current_pass_start: None,
            auto_sleep: false,
        }
    }

    /// Creates a governor with custom active threshold and cool-down period.
    ///
    /// Ideal for scaled-down sub-millisecond testing or specialized thermal profiles.
    pub fn with_thresholds(active_period_micros: u64, cool_period: Duration) -> Self {
        Self {
            active_period_micros: active_period_micros.max(1),
            cool_period,
            accumulated_active_micros: 0,
            total_active_micros: 0,
            total_cooldowns_triggered: 0,
            current_pass_start: None,
            auto_sleep: false,
        }
    }

    /// Sets whether the governor automatically invokes `std::thread::sleep` on threshold reach.
    pub fn with_auto_sleep(mut self, enabled: bool) -> Self {
        self.auto_sleep = enabled;
        self
    }

    /// Notifies the governor that a new active benchmark pass has commenced.
    #[inline]
    pub fn notify_pass_start(&mut self) {
        self.current_pass_start = Some(Instant::now());
    }

    /// Notifies the governor that the current active benchmark pass has ended.
    ///
    /// Accumulates elapsed active time and triggers cooldown if threshold is reached.
    pub fn notify_pass_end(&mut self) -> Option<Duration> {
        if let Some(start) = self.current_pass_start.take() {
            let elapsed_micros = start.elapsed().as_micros() as u64;
            self.record_active_micros(elapsed_micros)
        } else {
            None
        }
    }

    /// Records active execution duration in microseconds.
    ///
    /// If accumulated active duration reaches or exceeds `active_period_micros`,
    /// triggers a cool-down cycle, resets the current accumulator, and returns `Some(cool_period)`.
    pub fn record_active_micros(&mut self, micros: u64) -> Option<Duration> {
        self.accumulated_active_micros = self.accumulated_active_micros.saturating_add(micros);
        self.total_active_micros = self.total_active_micros.saturating_add(micros);

        if self.accumulated_active_micros >= self.active_period_micros {
            // Carry over any residual microseconds exceeding the threshold
            self.accumulated_active_micros %= self.active_period_micros;
            self.total_cooldowns_triggered += 1;

            if self.auto_sleep && !self.cool_period.is_zero() {
                std::thread::sleep(self.cool_period);
            }

            Some(self.cool_period)
        } else {
            None
        }
    }

    /// Records active duration in nanoseconds.
    #[inline]
    pub fn record_active_nanos(&mut self, nanos: u64) -> Option<Duration> {
        self.record_active_micros(nanos / 1_000)
    }

    /// Records active duration as a `Duration`.
    #[inline]
    pub fn record_active_duration(&mut self, duration: Duration) -> Option<Duration> {
        self.record_active_micros(duration.as_micros() as u64)
    }

    /// Checks if cooling is currently required.
    #[inline]
    pub fn is_cooling_needed(&self) -> bool {
        self.accumulated_active_micros >= self.active_period_micros
    }

    /// Returns the remaining active workload headroom in microseconds before the next cooling.
    #[inline]
    pub fn remaining_active_micros(&self) -> u64 {
        self.active_period_micros.saturating_sub(self.accumulated_active_micros)
    }

    /// Returns the accumulated active microseconds in the current cycle.
    #[inline]
    pub fn accumulated_micros(&self) -> u64 {
        self.accumulated_active_micros
    }

    /// Returns the total cumulative active microseconds tracked across all cycles.
    #[inline]
    pub fn total_active_micros(&self) -> u64 {
        self.total_active_micros
    }

    /// Returns total cooldown events triggered.
    #[inline]
    pub fn total_cooldowns_triggered(&self) -> usize {
        self.total_cooldowns_triggered
    }

    /// Resets the current active cycle accumulator to zero.
    #[inline]
    pub fn reset(&mut self) {
        self.accumulated_active_micros = 0;
        self.current_pass_start = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thermal_governor_constants() {
        assert_eq!(ACTIVE_PERIOD_MICROS, 70_000_000);
        assert_eq!(COOL_PERIOD_SECS, 10);
        assert_eq!(DEFAULT_COOL_PERIOD, Duration::from_secs(10));
    }

    #[test]
    fn test_thermal_governor_threshold_accumulation() {
        let mut gov = ThermalThrottleGovernor::new();
        assert_eq!(gov.accumulated_micros(), 0);
        assert_eq!(gov.remaining_active_micros(), 70_000_000);

        // Add 30s
        let trigger = gov.record_active_micros(30_000_000);
        assert!(trigger.is_none());
        assert_eq!(gov.accumulated_micros(), 30_000_000);
        assert_eq!(gov.remaining_active_micros(), 40_000_000);

        // Add 39s -> total 69s (no trigger)
        let trigger2 = gov.record_active_micros(39_000_000);
        assert!(trigger2.is_none());
        assert_eq!(gov.accumulated_micros(), 69_000_000);
        assert_eq!(gov.remaining_active_micros(), 1_000_000);

        // Add 2s -> total 71s (triggers cooldown of 10s, remainder 1s)
        let trigger3 = gov.record_active_micros(2_000_000);
        assert_eq!(trigger3, Some(Duration::from_secs(10)));
        assert_eq!(gov.accumulated_micros(), 1_000_000);
        assert_eq!(gov.total_cooldowns_triggered(), 1);
        assert_eq!(gov.total_active_micros(), 71_000_000);
    }

    #[test]
    fn test_thermal_governor_micro_step_simulation() {
        // Use 100 microseconds threshold and 1ms cooldown for fast unit test
        let mut gov = ThermalThrottleGovernor::with_thresholds(100, Duration::from_millis(1));

        for i in 0..10 {
            let res = gov.record_active_micros(25);
            if (i + 1) % 4 == 0 {
                assert_eq!(res, Some(Duration::from_millis(1)));
            } else {
                assert!(res.is_none());
            }
        }

        assert_eq!(gov.total_cooldowns_triggered(), 2);
        assert_eq!(gov.accumulated_micros(), 50);
        assert_eq!(gov.total_active_micros(), 250);
    }

    #[test]
    fn test_thermal_governor_pass_lifecycle() {
        let mut gov = ThermalThrottleGovernor::with_thresholds(50_000, Duration::from_millis(1));
        gov.notify_pass_start();
        std::thread::sleep(Duration::from_millis(2));
        let cool = gov.notify_pass_end();
        assert!(cool.is_none());
        assert!(gov.accumulated_micros() >= 1_000);

        // Record a larger chunk to exceed threshold
        let cool2 = gov.record_active_micros(60_000);
        assert_eq!(cool2, Some(Duration::from_millis(1)));
        assert_eq!(gov.total_cooldowns_triggered(), 1);
        assert!(gov.accumulated_micros() < 50_000);
    }

    #[test]
    fn test_thermal_governor_reset() {
        let mut gov = ThermalThrottleGovernor::new();
        gov.record_active_micros(50_000_000);
        assert_eq!(gov.accumulated_micros(), 50_000_000);

        gov.reset();
        assert_eq!(gov.accumulated_micros(), 0);
        assert_eq!(gov.total_active_micros(), 50_000_000); // Historical total preserved
    }
}
