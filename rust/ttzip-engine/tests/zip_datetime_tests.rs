// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use ttzip_engine::zip::datetime::{
    days_in_month, dos_to_unix_epoch_secs, is_leap_year, unix_epoch_secs_to_dos, DosDateTime,
};

#[test]
fn test_leap_year_rules() {
    // 1980 is divisible by 4 and not 100 -> leap
    assert!(is_leap_year(1980));
    // 1999 is not divisible by 4 -> common
    assert!(!is_leap_year(1999));
    // 2000 is divisible by 400 -> leap
    assert!(is_leap_year(2000));
    // 2024 is divisible by 4 and not 100 -> leap
    assert!(is_leap_year(2024));
    // 2026 is current year -> common
    assert!(!is_leap_year(2026));
    // 2100 is divisible by 100 and not 400 -> common (not leap!)
    assert!(!is_leap_year(2100));
    // 2104 is leap
    assert!(is_leap_year(2104));
}

#[test]
fn test_days_in_month_boundaries() {
    // February in leap vs non-leap years
    assert_eq!(days_in_month(1980, 2), 29);
    assert_eq!(days_in_month(2000, 2), 29);
    assert_eq!(days_in_month(2024, 2), 29);
    assert_eq!(days_in_month(2026, 2), 28);
    assert_eq!(days_in_month(2100, 2), 28);

    // Standard 30 and 31 day months
    assert_eq!(days_in_month(2026, 1), 31);
    assert_eq!(days_in_month(2026, 4), 30);
    assert_eq!(days_in_month(2026, 12), 31);

    // Invalid month numbers
    assert_eq!(days_in_month(2026, 0), 0);
    assert_eq!(days_in_month(2026, 13), 0);
}

#[test]
fn test_dos_datetime_min_boundary_1980() {
    let dt_min = DosDateTime::MIN;
    assert_eq!(dt_min.year, 1980);
    assert_eq!(dt_min.month, 1);
    assert_eq!(dt_min.day, 1);
    assert_eq!(dt_min.hour, 0);
    assert_eq!(dt_min.minute, 0);
    assert_eq!(dt_min.second, 0);

    let (date, time) = dt_min.to_dos();
    // 1980-01-01: year_offset=0, month=1, day=1 -> (0 << 9) | (1 << 5) | 1 = 0x0021
    // 00:00:00 -> 0x0000
    assert_eq!(date, 0x0021);
    assert_eq!(time, 0x0000);

    let parsed = DosDateTime::from_dos(date, time).expect("Failed to parse min DOS timestamp");
    assert_eq!(parsed, dt_min);

    // 1980-01-01 00:00:00 UTC epoch seconds is exactly 315532800
    let epoch = dt_min.to_unix_epoch_secs();
    assert_eq!(epoch, 315_532_800);

    let from_epoch = DosDateTime::from_unix_epoch_secs(epoch);
    assert_eq!(from_epoch, dt_min);
}

#[test]
fn test_dos_datetime_max_boundary_2107() {
    let dt_max = DosDateTime::MAX;
    assert_eq!(dt_max.year, 2107);
    assert_eq!(dt_max.month, 12);
    assert_eq!(dt_max.day, 31);
    assert_eq!(dt_max.hour, 23);
    assert_eq!(dt_max.minute, 59);
    assert_eq!(dt_max.second, 58);

    let (date, time) = dt_max.to_dos();
    // 2107-12-31: year_offset=127 (0x7F), month=12, day=31 -> (127 << 9) | (12 << 5) | 31 = 0xFF9F
    // 23:59:58 -> (23 << 11) | (59 << 5) | 29 = 0xBF7D
    assert_eq!(date, 0xFF9F);
    assert_eq!(time, 0xBF7D);

    let parsed = DosDateTime::from_dos(date, time).expect("Failed to parse max DOS timestamp");
    assert_eq!(parsed, dt_max);

    let epoch = dt_max.to_unix_epoch_secs();
    // 2107-12-31 23:59:58 UTC epoch seconds is 4354819198
    assert_eq!(epoch, 4_354_819_198);

    let from_epoch = DosDateTime::from_unix_epoch_secs(epoch);
    assert_eq!(from_epoch, dt_max);
}

#[test]
fn test_dos_datetime_current_timestamp_2026() {
    // Current target test date: 2026-08-30 14:45:02 UTC
    let dt = DosDateTime::new(2026, 8, 30, 14, 45, 2).expect("Valid timestamp");
    assert_eq!(dt.year, 2026);
    assert_eq!(dt.month, 8);
    assert_eq!(dt.day, 30);
    assert_eq!(dt.hour, 14);
    assert_eq!(dt.minute, 45);
    assert_eq!(dt.second, 2);

    let (date, time) = dt.to_dos();
    let parsed = DosDateTime::from_dos(date, time).expect("from_dos failed");
    assert_eq!(parsed, dt);

    let epoch = dt.to_unix_epoch_secs();
    let from_epoch = DosDateTime::from_unix_epoch_secs(epoch);
    assert_eq!(from_epoch, dt);

    // Verify 2-second resolution behavior on odd seconds
    let dt_odd = DosDateTime::new(2026, 8, 30, 14, 45, 3).expect("Valid timestamp with odd sec");
    assert_eq!(dt_odd.second, 2); // Aligned down to 2-second boundary
}

#[test]
fn test_dos_datetime_all_zero_safe_fallback() {
    // 0x0000, 0x0000 is common in uninitialized archive headers
    let fallback = DosDateTime::from_dos(0, 0);
    assert_eq!(fallback, Some(DosDateTime::MIN));

    // Standalone helper fallback
    let epoch = dos_to_unix_epoch_secs(0, 0);
    assert_eq!(epoch, DosDateTime::MIN.to_unix_epoch_secs());

    // Date == 0 with non-zero time
    // 10:30:00 -> (10 << 11) | (30 << 5) | 0 = 0x53C0
    let time_only = DosDateTime::from_dos(0, (10 << 11) | (30 << 5));
    assert_eq!(
        time_only,
        Some(DosDateTime {
            year: 1980,
            month: 1,
            day: 1,
            hour: 10,
            minute: 30,
            second: 0,
        })
    );
}

#[test]
fn test_dos_datetime_invalid_values_rejection() {
    // Invalid month 0 (with non-zero date)
    let invalid_month0 = (10 << 9) | 1;
    assert_eq!(DosDateTime::from_dos(invalid_month0, 0), None);

    // Invalid month 13
    let invalid_month13 = (10 << 9) | (13 << 5) | 1;
    assert_eq!(DosDateTime::from_dos(invalid_month13, 0), None);

    // Invalid day 0
    let invalid_day0 = (10 << 9) | (5 << 5);
    assert_eq!(DosDateTime::from_dos(invalid_day0, 0), None);

    // Invalid day 31 in 30-day month (April)
    let invalid_apr31 = (10 << 9) | (4 << 5) | 31;
    assert_eq!(DosDateTime::from_dos(invalid_apr31, 0), None);

    // Invalid day 29 in February of non-leap year (2026 is year_offset 46)
    let invalid_feb29_2026 = (46 << 9) | (2 << 5) | 29;
    assert_eq!(DosDateTime::from_dos(invalid_feb29_2026, 0), None);

    // Valid day 29 in February of leap year (2024 is year_offset 44)
    let valid_feb29_2024 = (44 << 9) | (2 << 5) | 29;
    assert!(DosDateTime::from_dos(valid_feb29_2024, 0).is_some());

    // Invalid hour 24
    let invalid_hour24 = 24 << 11;
    assert_eq!(DosDateTime::from_dos(0x0021, invalid_hour24), None);

    // Invalid minute 60
    let invalid_min60 = (12 << 11) | (60 << 5);
    assert_eq!(DosDateTime::from_dos(0x0021, invalid_min60), None);

    // Invalid second > 58 (raw_sec >= 30)
    let invalid_sec30 = (12 << 11) | (30 << 5) | 30;
    assert_eq!(DosDateTime::from_dos(0x0021, invalid_sec30), None);
}

#[test]
fn test_dos_datetime_epoch_clamping() {
    // Epoch before 1980 (e.g. Unix Epoch 0 = 1970-01-01) clamps to MIN (1980-01-01)
    let before_1980 = DosDateTime::from_unix_epoch_secs(0);
    assert_eq!(before_1980, DosDateTime::MIN);

    let negative_epoch = DosDateTime::from_unix_epoch_secs(-1000000);
    assert_eq!(negative_epoch, DosDateTime::MIN);

    // Epoch after 2107 clamps to MAX
    let far_future = DosDateTime::from_unix_epoch_secs(9_999_999_999);
    assert_eq!(far_future, DosDateTime::MAX);

    // Roundtrip conversion helper
    let (d, t) = unix_epoch_secs_to_dos(0);
    assert_eq!((d, t), DosDateTime::MIN.to_dos());
}
