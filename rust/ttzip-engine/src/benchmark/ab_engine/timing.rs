// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! High-Precision Nanosecond Timing and Hardware Clock Rising-Edge Alignment (`UTIL_waitForNextTick`).
//!
//! Provides ultra-low-overhead hardware monotonic clock probing and rising-edge
//! detection to eliminate initial 0..1 tick quantization jitter in micro-benchmarks.

use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
extern "C" {
    fn clock_gettime_nsec_np(clock_id: libc::clockid_t) -> u64;
}

/// Reads raw hardware monotonic clock in nanoseconds.
///
/// On macOS / Darwin (including aarch64 Apple Silicon), uses `clock_gettime_nsec_np(CLOCK_MONOTONIC_RAW)`.
/// On Linux and Unix platforms, uses `clock_gettime(CLOCK_MONOTONIC_RAW, ...)`.
/// Falls back to standard `Instant` on non-Unix platforms.
#[inline(always)]
pub fn get_hardware_monotonic_nanos() -> u64 {
    #[cfg(target_os = "macos")]
    {
        // Direct zero-syscall-overhead Mach/macOS monotonic raw clock in nanoseconds.
        unsafe { clock_gettime_nsec_np(libc::CLOCK_MONOTONIC_RAW) }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        unsafe {
            libc::clock_gettime(libc::CLOCK_MONOTONIC_RAW, &mut ts);
        }
        (ts.tv_sec as u64) * 1_000_000_000 + (ts.tv_nsec as u64)
    }
    #[cfg(not(unix))]
    {
        use std::sync::OnceLock;
        static EPOCH: OnceLock<Instant> = OnceLock::new();
        let epoch = EPOCH.get_or_init(Instant::now);
        epoch.elapsed().as_nanos() as u64
    }
}

/// Spins until the hardware monotonic clock timestamp strictly increments,
/// capturing the immediate rising edge to eliminate sub-tick initial quantization truncation.
///
/// Returns the timestamp in nanoseconds at the rising edge.
#[inline(always)]
pub fn wait_for_next_tick() -> u64 {
    let start = get_hardware_monotonic_nanos();
    loop {
        let current = get_hardware_monotonic_nanos();
        if current > start {
            return current;
        }
        std::hint::spin_loop();
    }
}

/// Aligns execution with the rising edge of `std::time::Instant`.
///
/// Spins until `Instant::now()` yields a strictly newer timestamp than the initial sample.
#[inline(always)]
pub fn wait_for_next_tick_instant() -> Instant {
    let start = Instant::now();
    loop {
        let now = Instant::now();
        if now > start {
            return now;
        }
        std::hint::spin_loop();
    }
}

/// Measures the minimum discernible hardware timer resolution by sampling consecutive ticks.
pub fn estimate_clock_resolution_nanos(samples: usize) -> f64 {
    let sample_count = samples.max(16);
    let mut min_delta = u64::MAX;

    for _ in 0..sample_count {
        let t1 = wait_for_next_tick();
        let t2 = wait_for_next_tick();
        let delta = t2.saturating_sub(t1);
        if delta > 0 && delta < min_delta {
            min_delta = delta;
        }
    }

    if min_delta == u64::MAX {
        1.0
    } else {
        min_delta as f64
    }
}

/// High-precision hardware monotonic stopwatch with rising-edge synchronization.
#[derive(Debug, Clone, Copy)]
pub struct HardwareMonotonicStopwatch {
    start_nanos: u64,
    lap_marker_nanos: u64,
}

impl Default for HardwareMonotonicStopwatch {
    fn default() -> Self {
        Self::new()
    }
}

impl HardwareMonotonicStopwatch {
    /// Creates and starts a new stopwatch aligned to the next hardware clock tick.
    #[inline]
    pub fn new() -> Self {
        let start = wait_for_next_tick();
        Self {
            start_nanos: start,
            lap_marker_nanos: start,
        }
    }

    /// Creates and starts a new stopwatch immediately without rising-edge wait.
    #[inline]
    pub fn new_unaligned() -> Self {
        let now = get_hardware_monotonic_nanos();
        Self {
            start_nanos: now,
            lap_marker_nanos: now,
        }
    }

    /// Resets the stopwatch with rising-edge alignment.
    #[inline]
    pub fn reset_aligned(&mut self) {
        let start = wait_for_next_tick();
        self.start_nanos = start;
        self.lap_marker_nanos = start;
    }

    /// Returns elapsed nanoseconds since start.
    #[inline]
    pub fn elapsed_nanos(&self) -> u64 {
        let current = get_hardware_monotonic_nanos();
        current.saturating_sub(self.start_nanos)
    }

    /// Returns elapsed microseconds as `f64`.
    #[inline]
    pub fn elapsed_micros(&self) -> f64 {
        self.elapsed_nanos() as f64 / 1_000.0
    }

    /// Returns elapsed milliseconds as `f64`.
    #[inline]
    pub fn elapsed_millis(&self) -> f64 {
        self.elapsed_nanos() as f64 / 1_000_000.0
    }

    /// Returns elapsed seconds as `f64`.
    #[inline]
    pub fn elapsed_secs(&self) -> f64 {
        self.elapsed_nanos() as f64 / 1_000_000_000.0
    }

    /// Returns elapsed duration as standard `Duration`.
    #[inline]
    pub fn elapsed_duration(&self) -> Duration {
        Duration::from_nanos(self.elapsed_nanos())
    }

    /// Records lap duration in nanoseconds and advances lap marker.
    #[inline]
    pub fn lap_nanos(&mut self) -> u64 {
        let current = get_hardware_monotonic_nanos();
        let delta = current.saturating_sub(self.lap_marker_nanos);
        self.lap_marker_nanos = current;
        delta
    }
}

/// Executes a closure timed with hardware clock rising-edge alignment.
///
/// Returns a tuple `(result, elapsed_nanos)`.
#[inline]
pub fn time_aligned_closure<F, R>(mut f: F) -> (R, u64)
where
    F: FnMut() -> R,
{
    let start = wait_for_next_tick();
    let result = f();
    let end = get_hardware_monotonic_nanos();
    let elapsed = end.saturating_sub(start).max(1);
    (result, elapsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardware_monotonic_nanos_strictly_advances() {
        let t1 = get_hardware_monotonic_nanos();
        assert!(t1 > 0);
        let t2 = wait_for_next_tick();
        assert!(t2 > t1);
    }

    #[test]
    fn test_wait_for_next_tick_rising_edge() {
        let t1 = wait_for_next_tick();
        let t2 = wait_for_next_tick();
        assert!(t2 > t1);
    }

    #[test]
    fn test_wait_for_next_tick_instant() {
        let i1 = wait_for_next_tick_instant();
        let i2 = wait_for_next_tick_instant();
        assert!(i2 > i1);
    }

    #[test]
    fn test_hardware_monotonic_stopwatch() {
        let sw = HardwareMonotonicStopwatch::new();
        std::thread::sleep(Duration::from_millis(5));
        let nanos = sw.elapsed_nanos();
        assert!(nanos >= 4_000_000);
        assert!(sw.elapsed_millis() >= 4.0);
    }

    #[test]
    fn test_time_aligned_closure() {
        let (val, elapsed) = time_aligned_closure(|| {
            let mut acc = 0u64;
            for i in 0..100_000 {
                acc = acc.wrapping_add(std::hint::black_box(i));
            }
            acc
        });
        assert!(val > 0);
        assert!(elapsed > 0);
    }

    #[test]
    fn test_estimate_clock_resolution() {
        let res = estimate_clock_resolution_nanos(32);
        assert!(res > 0.0);
    }
}
