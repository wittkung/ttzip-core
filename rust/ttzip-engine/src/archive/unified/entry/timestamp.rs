// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! High-precision nanosecond timestamp representation and epoch conversions.

use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// High-precision timestamp with nanosecond resolution.
///
/// Implements POSIX normalized timespec invariants: `0 <= nsec < 1_000_000_000`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TTZipTimestamp {
    pub sec: i64,
    pub nsec: u32,
}

impl TTZipTimestamp {
    pub const NANOS_PER_SEC: u32 = 1_000_000_000;

    /// Creates a normalized timestamp handling potential nanosecond overflow.
    pub fn new(sec: i64, nsec: u32) -> Self {
        if nsec < Self::NANOS_PER_SEC {
            Self { sec, nsec }
        } else {
            let carry = (nsec / Self::NANOS_PER_SEC) as i64;
            let rem = nsec % Self::NANOS_PER_SEC;
            Self {
                sec: sec.saturating_add(carry),
                nsec: rem,
            }
        }
    }

    /// Creates a timestamp from signed nanoseconds since UNIX epoch.
    pub fn from_nanos_signed(total_nanos: i128) -> Self {
        let nanos_per_sec = Self::NANOS_PER_SEC as i128;
        let sec = total_nanos.div_euclid(nanos_per_sec) as i64;
        let nsec = total_nanos.rem_euclid(nanos_per_sec) as u32;
        Self { sec, nsec }
    }

    /// Converts the timestamp to total signed nanoseconds since UNIX epoch.
    #[inline]
    pub fn as_total_nanos(&self) -> i128 {
        (self.sec as i128) * (Self::NANOS_PER_SEC as i128) + (self.nsec as i128)
    }

    /// Creates a timestamp from epoch whole seconds.
    #[inline]
    pub const fn from_epoch_secs(sec: i64) -> Self {
        Self { sec, nsec: 0 }
    }

    /// Creates a timestamp from epoch milliseconds.
    pub fn from_epoch_millis(millis: i64) -> Self {
        let sec = millis.div_euclid(1_000);
        let rem_ms = millis.rem_euclid(1_000) as u32;
        Self {
            sec,
            nsec: rem_ms * 1_000_000,
        }
    }

    /// Converts standard `SystemTime` to `TTZipTimestamp`.
    pub fn from_system_time(st: SystemTime) -> Self {
        match st.duration_since(UNIX_EPOCH) {
            Ok(duration) => Self::new(duration.as_secs() as i64, duration.subsec_nanos()),
            Err(before_epoch) => {
                let dur = before_epoch.duration();
                let dur_secs = dur.as_secs() as i64;
                let dur_nanos = dur.subsec_nanos();
                if dur_nanos == 0 {
                    Self {
                        sec: -dur_secs,
                        nsec: 0,
                    }
                } else {
                    Self {
                        sec: -dur_secs - 1,
                        nsec: Self::NANOS_PER_SEC - dur_nanos,
                    }
                }
            }
        }
    }

    /// Converts to standard `SystemTime`, if within representable range.
    pub fn to_system_time(&self) -> Option<SystemTime> {
        if self.sec >= 0 {
            UNIX_EPOCH.checked_add(Duration::new(self.sec as u64, self.nsec))
        } else {
            let total_nanos = self.as_total_nanos();
            if total_nanos < 0 {
                let abs_nanos = (-total_nanos) as u64;
                let secs = abs_nanos / (Self::NANOS_PER_SEC as u64);
                let nanos = (abs_nanos % (Self::NANOS_PER_SEC as u64)) as u32;
                UNIX_EPOCH.checked_sub(Duration::new(secs, nanos))
            } else {
                UNIX_EPOCH.checked_add(Duration::from_nanos(total_nanos as u64))
            }
        }
    }

    #[inline]
    pub const fn epoch_secs(&self) -> i64 {
        self.sec
    }

    #[inline]
    pub fn epoch_millis(&self) -> i64 {
        (self.as_total_nanos().div_euclid(1_000_000)) as i64
    }
}

impl fmt::Display for TTZipTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:09}s", self.sec, self.nsec)
    }
}
