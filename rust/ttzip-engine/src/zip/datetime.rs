// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! MS-DOS 16-bit FAT / PKZIP timestamp bidirectional conversion.
//!
//! Provides deterministic conversions between MS-DOS packed 16-bit date/time integers
//! and standard UTC Unix epoch timestamps (seconds since 1970-01-01 00:00:00 UTC).
//! Accurately handles the 1980..2107 valid range, 2-second resolution, leap years,
//! and safe all-zero fallback.

/// Number of seconds in a standard day.
const SECONDS_PER_DAY: i64 = 86_400;

/// Days in each month for non-leap and leap years.
const DAYS_PER_MONTH: [[u8; 12]; 2] = [
    [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31],
    [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31],
];

/// Returns `true` if `year` is a leap year in the Gregorian calendar.
#[inline]
#[allow(clippy::manual_is_multiple_of)]
pub const fn is_leap_year(year: u16) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Returns the number of days in the specified month of a year.
#[inline]
pub const fn days_in_month(year: u16, month: u8) -> u8 {
    if month < 1 || month > 12 {
        return 0;
    }
    let leap_idx = if is_leap_year(year) { 1 } else { 0 };
    DAYS_PER_MONTH[leap_idx][(month - 1) as usize]
}

/// MS-DOS unpacked calendar date and time representation.
///
/// MS-DOS timestamps have a 2-second resolution (`second` is always an even integer `0..=58`)
/// and represent years between `1980` and `2107` inclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DosDateTime {
    /// Gregorian year (`1980..=2107`).
    pub year: u16,
    /// Month of the year (`1..=12`).
    pub month: u8,
    /// Day of the month (`1..=31`).
    pub day: u8,
    /// Hour of the day (`0..=23`).
    pub hour: u8,
    /// Minute of the hour (`0..=59`).
    pub minute: u8,
    /// Second of the minute (`0..=58`, 2-second resolution).
    pub second: u8,
}

impl Default for DosDateTime {
    #[inline]
    fn default() -> Self {
        Self::MIN
    }
}

impl DosDateTime {
    /// Earliest valid MS-DOS timestamp: `1980-01-01 00:00:00`.
    pub const MIN: Self = Self {
        year: 1980,
        month: 1,
        day: 1,
        hour: 0,
        minute: 0,
        second: 0,
    };

    /// Latest valid MS-DOS timestamp: `2107-12-31 23:59:58`.
    pub const MAX: Self = Self {
        year: 2107,
        month: 12,
        day: 31,
        hour: 23,
        minute: 59,
        second: 58,
    };

    /// Creates a validated `DosDateTime` from calendar components.
    ///
    /// Seconds are aligned down to the nearest 2-second boundary.
    pub fn new(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Option<Self> {
        if !(1980..=2107).contains(&year) {
            return None;
        }
        if !(1..=12).contains(&month) {
            return None;
        }
        let max_days = days_in_month(year, month);
        if day < 1 || day > max_days {
            return None;
        }
        if hour > 23 || minute > 59 || second > 59 {
            return None;
        }

        Some(Self {
            year,
            month,
            day,
            hour,
            minute,
            second: second & !1,
        })
    }

    /// Parses an MS-DOS packed 16-bit `(date, time)` pair.
    ///
    /// # Bit Allocations
    /// - `date` (16 bits):
    ///   - Bits 0..=4: Day of month (`1..=31`)
    ///   - Bits 5..=8: Month (`1..=12`)
    ///   - Bits 9..=15: Year offset from 1980 (`0..=127` -> `1980..=2107`)
    /// - `time` (16 bits):
    ///   - Bits 0..=4: Seconds divided by 2 (`0..=29` -> `0..=58` s)
    ///   - Bits 5..=10: Minutes (`0..=59`)
    ///   - Bits 11..=15: Hours (`0..=23`)
    ///
    /// # Safe Fallback
    /// If both `date == 0` and `time == 0`, returns `Some(DosDateTime::MIN)` (1980-01-01 00:00:00).
    /// If `date == 0` but time is non-zero, defaults the date to `1980-01-01` and parses time.
    pub fn from_dos(date: u16, time: u16) -> Option<Self> {
        if date == 0 && time == 0 {
            return Some(Self::MIN);
        }

        let raw_sec = (time & 0x1F) as u8;
        let minute = ((time >> 5) & 0x3F) as u8;
        let hour = ((time >> 11) & 0x1F) as u8;

        if hour > 23 || minute > 59 || raw_sec > 29 {
            return None;
        }
        let second = raw_sec * 2;

        if date == 0 {
            return Some(Self {
                year: 1980,
                month: 1,
                day: 1,
                hour,
                minute,
                second,
            });
        }

        let day = (date & 0x1F) as u8;
        let month = ((date >> 5) & 0x0F) as u8;
        let year_offset = (date >> 9) & 0x7F;
        let year = 1980 + year_offset;

        if !(1..=12).contains(&month) {
            return None;
        }
        let max_days = days_in_month(year, month);
        if day < 1 || day > max_days {
            return None;
        }

        Some(Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        })
    }

    /// Encodes this `DosDateTime` into packed MS-DOS `(date, time)` 16-bit integers.
    #[inline]
    pub fn to_dos(&self) -> (u16, u16) {
        (self.dos_date(), self.dos_time())
    }

    /// Packed MS-DOS 16-bit date integer.
    #[inline]
    pub fn dos_date(&self) -> u16 {
        let year_offset = self.year.saturating_sub(1980).min(127);
        let month = (self.month as u16) & 0x0F;
        let day = (self.day as u16) & 0x1F;
        (year_offset << 9) | (month << 5) | day
    }

    /// Packed MS-DOS 16-bit time integer.
    #[inline]
    pub fn dos_time(&self) -> u16 {
        let hour = (self.hour as u16) & 0x1F;
        let minute = (self.minute as u16) & 0x3F;
        let sec_div_2 = ((self.second / 2).min(29) as u16) & 0x1F;
        (hour << 11) | (minute << 5) | sec_div_2
    }


    /// Converts this `DosDateTime` into seconds since Unix Epoch (1970-01-01 00:00:00 UTC).
    pub fn to_unix_epoch_secs(&self) -> i64 {
        let mut days = 0i64;

        // Days from 1970 up to this year
        for y in 1970..self.year {
            days += if is_leap_year(y) { 366 } else { 365 };
        }

        // Days in preceding months of current year
        let leap_idx = if is_leap_year(self.year) { 1 } else { 0 };
        for m in 1..self.month {
            days += DAYS_PER_MONTH[leap_idx][(m - 1) as usize] as i64;
        }

        // Days in current month (1-based -> 0-based offset)
        days += (self.day as i64) - 1;

        let seconds_of_day = (self.hour as i64) * 3600
            + (self.minute as i64) * 60
            + (self.second as i64);

        days * SECONDS_PER_DAY + seconds_of_day
    }

    /// Constructs a `DosDateTime` from Unix epoch seconds (UTC).
    ///
    /// Timestamps before `1980-01-01 00:00:00 UTC` are clamped to `Self::MIN`.
    /// Timestamps after `2107-12-31 23:59:58 UTC` are clamped to `Self::MAX`.
    pub fn from_unix_epoch_secs(secs: i64) -> Self {
        // Clamp to valid MS-DOS range [1980-01-01 00:00:00, 2107-12-31 23:59:58]
        let min_secs = Self::MIN.to_unix_epoch_secs();
        let max_secs = Self::MAX.to_unix_epoch_secs();

        if secs <= min_secs {
            return Self::MIN;
        }
        if secs >= max_secs {
            return Self::MAX;
        }

        let mut days_remaining = secs / SECONDS_PER_DAY;
        let mut seconds_of_day = (secs % SECONDS_PER_DAY) as u32;

        // Determine year starting from 1970
        let mut year = 1970u16;
        loop {
            let days_in_cur_year = if is_leap_year(year) { 366 } else { 365 };
            if days_remaining < days_in_cur_year {
                break;
            }
            days_remaining -= days_in_cur_year;
            year += 1;
        }

        // Determine month
        let leap_idx = if is_leap_year(year) { 1 } else { 0 };
        let mut month = 1u8;
        for &dim in &DAYS_PER_MONTH[leap_idx] {
            if days_remaining < dim as i64 {
                break;
            }
            days_remaining -= dim as i64;
            month += 1;
        }

        let day = (days_remaining + 1) as u8;
        let hour = (seconds_of_day / 3600) as u8;
        seconds_of_day %= 3600;
        let minute = (seconds_of_day / 60) as u8;
        let second = ((seconds_of_day % 60) & !1) as u8;

        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }
}

/// Convenience helper to convert packed MS-DOS timestamp to Unix epoch seconds.
#[inline]
pub fn dos_to_unix_epoch_secs(date: u16, time: u16) -> i64 {
    DosDateTime::from_dos(date, time)
        .map(|dt| dt.to_unix_epoch_secs())
        .unwrap_or(0)
}

/// Convenience helper to convert Unix epoch seconds to packed MS-DOS `(date, time)`.
#[inline]
pub fn unix_epoch_secs_to_dos(secs: i64) -> (u16, u16) {
    DosDateTime::from_unix_epoch_secs(secs).to_dos()
}
