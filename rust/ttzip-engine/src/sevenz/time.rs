// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Windows NT FILETIME (100ns intervals since 1601-01-01 UTC) and SystemTime conversions.
//!
//! Provides lossless bi-directional timestamp conversions between Windows NT time
//! and POSIX/Rust [`SystemTime`] with 100-nanosecond precision.

use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Number of 100-nanosecond intervals (ticks) per second.
pub const TICKS_PER_SEC: u64 = 10_000_000;

/// Number of nanoseconds per 100-nanosecond interval (tick).
pub const NANOS_PER_TICK: u64 = 100;

/// Number of 100-nanosecond ticks between Windows NT epoch (1601-01-01 00:00:00 UTC)
/// and Unix epoch (1970-01-01 00:00:00 UTC).
///
/// Equals 134,774 days = 11,644,473,600 seconds = 116,444,736,000,000,000 ticks.
pub const UNIX_EPOCH_TICKS: u64 = 116_444_736_000_000_000;

/// Number of seconds between Windows NT epoch (1601-01-01 00:00:00 UTC) and Unix epoch (1970-01-01 00:00:00 UTC).
pub const SECS_BETWEEN_1601_AND_1970: u64 = 11_644_473_600;

/// Number of nanoseconds in one whole second.
pub const NANOS_PER_SEC: u32 = 1_000_000_000;

/// Windows NT time representation (FILETIME).
///
/// Represents the number of 100-nanosecond intervals elapsed since
/// January 1, 1601 00:00:00 UTC (Coordinated Universal Time).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct NtTime(pub u64);

impl NtTime {
    /// Creates a new `NtTime` with the given raw 100ns tick count.
    #[inline]
    pub const fn new(ticks: u64) -> Self {
        Self(ticks)
    }

    /// Returns the raw 100-nanosecond tick count.
    #[inline]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    /// Returns `true` if this timestamp is zero (corresponding to 1601-01-01 00:00:00 UTC).
    #[inline]
    pub const fn is_zero(&self) -> bool {
        self.0 == 0
    }

    /// Converts this `NtTime` to standard Rust [`SystemTime`].
    ///
    /// Accurately handles timestamps both before and after the Unix epoch (1970-01-01 00:00:00 UTC),
    /// down to 100-nanosecond precision.
    pub fn to_system_time(&self) -> SystemTime {
        if self.0 >= UNIX_EPOCH_TICKS {
            let diff_ticks = self.0 - UNIX_EPOCH_TICKS;
            let secs = diff_ticks / TICKS_PER_SEC;
            let subsec_ticks = diff_ticks % TICKS_PER_SEC;
            let subsec_nanos = (subsec_ticks * NANOS_PER_TICK) as u32;
            UNIX_EPOCH
                .checked_add(Duration::new(secs, subsec_nanos))
                .unwrap_or(SystemTime::UNIX_EPOCH)
        } else {
            let diff_ticks = UNIX_EPOCH_TICKS - self.0;
            let secs = diff_ticks / TICKS_PER_SEC;
            let subsec_ticks = diff_ticks % TICKS_PER_SEC;
            let subsec_nanos = (subsec_ticks * NANOS_PER_TICK) as u32;
            UNIX_EPOCH
                .checked_sub(Duration::new(secs, subsec_nanos))
                .unwrap_or(SystemTime::UNIX_EPOCH)
        }
    }

    /// Converts a standard Rust [`SystemTime`] to an [`NtTime`].
    ///
    /// Returns `None` if the timestamp represents a point in time before the
    /// Windows NT epoch (1601-01-01 00:00:00 UTC) or if the tick count overflows `u64::MAX`.
    pub fn from_system_time(st: SystemTime) -> Option<Self> {
        match st.duration_since(UNIX_EPOCH) {
            Ok(duration) => {
                let secs = duration.as_secs();
                let nanos = duration.subsec_nanos();
                let secs_ticks = secs.checked_mul(TICKS_PER_SEC)?;
                let nanos_ticks = (nanos as u64) / NANOS_PER_TICK;
                let total_since_epoch = secs_ticks.checked_add(nanos_ticks)?;
                let total_ticks = UNIX_EPOCH_TICKS.checked_add(total_since_epoch)?;
                Some(Self(total_ticks))
            }
            Err(before_epoch) => {
                let dur = before_epoch.duration();
                let dur_secs = dur.as_secs();
                let dur_nanos = dur.subsec_nanos();
                let secs_ticks = dur_secs.checked_mul(TICKS_PER_SEC)?;
                let nanos_ticks = (dur_nanos as u64) / NANOS_PER_TICK;
                let total_before_epoch = secs_ticks.checked_add(nanos_ticks)?;
                if total_before_epoch > UNIX_EPOCH_TICKS {
                    // Date is before 1601-01-01 00:00:00 UTC
                    None
                } else {
                    Some(Self(UNIX_EPOCH_TICKS - total_before_epoch))
                }
            }
        }
    }

    /// Converts this `NtTime` to Unix epoch seconds and sub-second nanoseconds `(i64, u32)`.
    ///
    /// The returned sub-second nanoseconds are normalized to satisfy POSIX timespec invariants:
    /// `0 <= nanos < 1_000_000_000`.
    pub fn to_unix_secs_and_nanos(&self) -> (i64, u32) {
        if self.0 >= UNIX_EPOCH_TICKS {
            let diff_ticks = self.0 - UNIX_EPOCH_TICKS;
            let secs = (diff_ticks / TICKS_PER_SEC) as i64;
            let subsec_ticks = (diff_ticks % TICKS_PER_SEC) as u32;
            let subsec_nanos = subsec_ticks * (NANOS_PER_TICK as u32);
            (secs, subsec_nanos)
        } else {
            let diff_ticks = UNIX_EPOCH_TICKS - self.0;
            let secs_diff = (diff_ticks / TICKS_PER_SEC) as i64;
            let subsec_ticks = (diff_ticks % TICKS_PER_SEC) as u32;
            if subsec_ticks == 0 {
                (-secs_diff, 0)
            } else {
                let nsec = NANOS_PER_SEC - subsec_ticks * (NANOS_PER_TICK as u32);
                let sec = -secs_diff - 1;
                (sec, nsec)
            }
        }
    }

    /// Creates an `NtTime` from Unix epoch seconds and sub-second nanoseconds.
    ///
    /// Returns `None` if the resulting timestamp is before 1601-01-01 or overflows `u64::MAX`.
    pub fn from_unix_secs_and_nanos(sec: i64, nsec: u32) -> Option<Self> {
        let normalized_sec = sec.checked_add((nsec / NANOS_PER_SEC) as i64)?;
        let normalized_nsec = nsec % NANOS_PER_SEC;

        if normalized_sec >= 0 {
            let secs_ticks = (normalized_sec as u64).checked_mul(TICKS_PER_SEC)?;
            let nanos_ticks = (normalized_nsec as u64) / NANOS_PER_TICK;
            let total_since_epoch = secs_ticks.checked_add(nanos_ticks)?;
            let total_ticks = UNIX_EPOCH_TICKS.checked_add(total_since_epoch)?;
            Some(Self(total_ticks))
        } else {
            let abs_sec = (-normalized_sec) as u64;
            let secs_ticks = abs_sec.checked_mul(TICKS_PER_SEC)?;
            let nanos_ticks = (normalized_nsec as u64) / NANOS_PER_TICK;
            let total_before_epoch = secs_ticks.checked_sub(nanos_ticks)?;
            if total_before_epoch > UNIX_EPOCH_TICKS {
                None
            } else {
                Some(Self(UNIX_EPOCH_TICKS - total_before_epoch))
            }
        }
    }

    /// Creates an `NtTime` from Unix epoch seconds with zero sub-second nanoseconds.
    #[inline]
    pub fn from_unix_secs(sec: i64) -> Option<Self> {
        Self::from_unix_secs_and_nanos(sec, 0)
    }
}

impl From<u64> for NtTime {
    #[inline]
    fn from(ticks: u64) -> Self {
        Self(ticks)
    }
}

impl From<NtTime> for u64 {
    #[inline]
    fn from(nt: NtTime) -> Self {
        nt.0
    }
}

impl fmt::Display for NtTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NtTime({})", self.0)
    }
}
