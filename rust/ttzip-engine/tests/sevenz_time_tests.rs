// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Integration tests for Windows NT 100ns NtTime and SystemTime bi-directional conversions.

use std::time::{Duration, UNIX_EPOCH};
use ttzip_engine::sevenz::time::{
    NtTime, NANOS_PER_TICK, SECS_BETWEEN_1601_AND_1970, TICKS_PER_SEC, UNIX_EPOCH_TICKS,
};

#[test]
fn test_nt_time_unix_epoch_exact_value() {
    // 1. Unix Epoch (1970-01-01 00:00:00 UTC) must equal 116444736000000000 ticks.
    let expected_ticks: u64 = 116_444_736_000_000_000;
    assert_eq!(UNIX_EPOCH_TICKS, expected_ticks);
    assert_eq!(SECS_BETWEEN_1601_AND_1970, 11_644_473_600);
    assert_eq!(TICKS_PER_SEC, 10_000_000);
    assert_eq!(NANOS_PER_TICK, 100);

    let nt = NtTime(expected_ticks);
    assert_eq!(nt.to_system_time(), UNIX_EPOCH);
    assert_eq!(nt.to_unix_secs_and_nanos(), (0, 0));

    let from_st = NtTime::from_system_time(UNIX_EPOCH);
    assert_eq!(from_st, Some(nt));

    let from_unix = NtTime::from_unix_secs_and_nanos(0, 0);
    assert_eq!(from_unix, Some(nt));

    let from_secs = NtTime::from_unix_secs(0);
    assert_eq!(from_secs, Some(nt));
}

#[test]
fn test_nt_time_windows_filetime_epoch_zero() {
    // 2. Windows FileTime Epoch (1601-01-01 00:00:00 UTC) must equal NtTime(0).
    let nt = NtTime(0);
    assert!(nt.is_zero());
    assert_eq!(nt.as_u64(), 0);

    let (secs, nanos) = nt.to_unix_secs_and_nanos();
    assert_eq!(secs, -11_644_473_600);
    assert_eq!(nanos, 0);

    let expected_st = UNIX_EPOCH - Duration::from_secs(11_644_473_600);
    assert_eq!(nt.to_system_time(), expected_st);

    let recovered_from_st = NtTime::from_system_time(expected_st);
    assert_eq!(recovered_from_st, Some(nt));

    let recovered_from_unix = NtTime::from_unix_secs_and_nanos(-11_644_473_600, 0);
    assert_eq!(recovered_from_unix, Some(nt));

    let recovered_from_secs = NtTime::from_unix_secs(-11_644_473_600);
    assert_eq!(recovered_from_secs, Some(nt));
}

#[test]
fn test_nt_time_modern_timestamp_roundtrip_2026_08_30() {
    // 3. Modern timestamp: 2026-08-30 13:37:18.123456700 UTC.
    // 2026-08-30 00:00:00 UTC = 20,695 days * 86,400s = 1,788,048,000s since Unix Epoch.
    // 13:37:18 UTC = 49,038s.
    // Total Unix seconds = 1,788,097,038s.
    let unix_secs: i64 = 1_788_097_038;
    let sub_nanos: u32 = 123_456_700; // 1,234,567 ticks of 100ns

    let st = UNIX_EPOCH + Duration::new(unix_secs as u64, sub_nanos);
    let nt = NtTime::from_system_time(st).expect("convert from modern SystemTime");

    // Verify raw ticks calculation
    let expected_delta_ticks = (unix_secs as u64) * TICKS_PER_SEC + (sub_nanos as u64 / NANOS_PER_TICK);
    let expected_total_ticks = UNIX_EPOCH_TICKS + expected_delta_ticks;
    assert_eq!(nt.as_u64(), expected_total_ticks);

    // Verify lossless roundtrip to SystemTime
    let roundtrip_st = nt.to_system_time();
    assert_eq!(roundtrip_st, st);

    // Verify lossless roundtrip to Unix secs and nanos
    let (recovered_secs, recovered_nanos) = nt.to_unix_secs_and_nanos();
    assert_eq!(recovered_secs, unix_secs);
    assert_eq!(recovered_nanos, sub_nanos);

    // Verify sub-100ns precision truncation behavior (e.g. 123_456_789 ns -> 123_456_700 ns)
    let fine_nanos = 123_456_789;
    let st_fine = UNIX_EPOCH + Duration::new(unix_secs as u64, fine_nanos);
    let nt_fine = NtTime::from_system_time(st_fine).expect("convert fine SystemTime");
    assert_eq!(nt_fine, nt);
    assert_eq!(nt_fine.to_system_time(), st);
}

#[test]
fn test_nt_time_boundaries_and_overflow_safety() {
    // 4. Boundary safety and u64::MAX bound verification
    let max_nt = NtTime(u64::MAX);
    assert_eq!(max_nt.as_u64(), u64::MAX);
    assert!(!max_nt.is_zero());

    let max_st = max_nt.to_system_time();
    let recovered_max = NtTime::from_system_time(max_st);
    assert_eq!(recovered_max, Some(max_nt));

    let (max_secs, max_nanos) = max_nt.to_unix_secs_and_nanos();
    let diff = u64::MAX - UNIX_EPOCH_TICKS;
    assert_eq!(max_secs, (diff / TICKS_PER_SEC) as i64);
    assert_eq!(max_nanos, ((diff % TICKS_PER_SEC) * NANOS_PER_TICK) as u32);

    // Verify date before 1601-01-01 00:00:00 UTC returns None safely
    let before_1601_st = UNIX_EPOCH - Duration::from_secs(11_644_473_601);
    assert_eq!(NtTime::from_system_time(before_1601_st), None);
    assert_eq!(NtTime::from_unix_secs_and_nanos(-11_644_473_601, 0), None);
    assert_eq!(NtTime::from_unix_secs(-11_644_473_601), None);

    // Verify extreme future timestamp beyond u64::MAX ticks returns None safely
    let far_future_secs = u64::MAX / (TICKS_PER_SEC / 2);
    let far_future_st = UNIX_EPOCH + Duration::from_secs(far_future_secs);
    assert_eq!(NtTime::from_system_time(far_future_st), None);
}

#[test]
fn test_nt_time_pre_epoch_subsecond_normalization() {
    // 5. Test sub-second normalization before Unix epoch (1970)
    // Exactly 100ns before Unix Epoch (1970-01-01 00:00:00 UTC)
    let nt_pre_1 = NtTime(UNIX_EPOCH_TICKS - 1);
    let (sec_pre_1, nsec_pre_1) = nt_pre_1.to_unix_secs_and_nanos();
    assert_eq!(sec_pre_1, -1);
    assert_eq!(nsec_pre_1, 999_999_900);

    let st_pre_1 = UNIX_EPOCH - Duration::new(0, 100);
    assert_eq!(nt_pre_1.to_system_time(), st_pre_1);
    assert_eq!(NtTime::from_system_time(st_pre_1), Some(nt_pre_1));
    assert_eq!(NtTime::from_unix_secs_and_nanos(-1, 999_999_900), Some(nt_pre_1));

    // Exactly 100ns after Unix Epoch
    let nt_post_1 = NtTime(UNIX_EPOCH_TICKS + 1);
    let (sec_post_1, nsec_post_1) = nt_post_1.to_unix_secs_and_nanos();
    assert_eq!(sec_post_1, 0);
    assert_eq!(nsec_post_1, 100);

    let st_post_1 = UNIX_EPOCH + Duration::new(0, 100);
    assert_eq!(nt_post_1.to_system_time(), st_post_1);
    assert_eq!(NtTime::from_system_time(st_post_1), Some(nt_post_1));
    assert_eq!(NtTime::from_unix_secs_and_nanos(0, 100), Some(nt_post_1));

    // Exactly 1 tick after Windows NT epoch (1601-01-01 00:00:00.000000100 UTC)
    let nt_1601_plus_1 = NtTime(1);
    let (sec_1601, nsec_1601) = nt_1601_plus_1.to_unix_secs_and_nanos();
    assert_eq!(sec_1601, -11_644_473_600);
    assert_eq!(nsec_1601, 100);

    let expected_1601_st = UNIX_EPOCH - Duration::new(11_644_473_599, 999_999_900);
    assert_eq!(nt_1601_plus_1.to_system_time(), expected_1601_st);
    assert_eq!(NtTime::from_system_time(expected_1601_st), Some(nt_1601_plus_1));
    assert_eq!(NtTime::from_unix_secs_and_nanos(-11_644_473_600, 100), Some(nt_1601_plus_1));
}

#[test]
fn test_nt_time_traits_and_conversions() {
    // 6. Conversions and trait implementations
    let default_nt = NtTime::default();
    assert_eq!(default_nt.as_u64(), 0);
    assert!(default_nt.is_zero());

    let nt = NtTime::new(1234567890);
    let raw: u64 = nt.into();
    assert_eq!(raw, 1234567890);

    let back: NtTime = raw.into();
    assert_eq!(back, nt);

    let formatted = format!("{}", nt);
    assert_eq!(formatted, "NtTime(1234567890)");
}
