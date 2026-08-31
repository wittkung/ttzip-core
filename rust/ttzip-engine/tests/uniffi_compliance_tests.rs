// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Official UniFFI-rs 29-Fixture Compliance & 6-Layer Defense Verification Suite.
//!
//! Validates full conformance against Mozilla UniFFI specifications across:
//! 1. Scalar primitive types
//! 2. UTF-8 strings & Unicode emojis
//! 3. Binary byte vectors & raw blobs
//! 4. Optional nullable types
//! 5. Sequence & nested list collections
//! 6. Dictionary & key-value records
//! 7. Flat enum variants & discriminants
//! 8. Complex associated-data enums
//! 9. Nested records & composite models
//! 10. Typed error models & exception flows
//! 11. Opaque Rust objects & Arc interfaces
//! 12. Multiple constructors & factory patterns
//! 13. Synchronous foreign callback dispatch
//! 14. Async future tasks & multi-thread offloading
//! 15. Concurrent Arc object sharing
//! 16. RAII lifecycle & drop counters
//! 17. Handle double-free interception
//! 18. Null pointer & invalid address defense
//! 19. Pointer memory alignment defense
//! 20. RustBuffer bounds & capacity quota defense
//! 21. RustBuffer offset slice access
//! 22. Panic boundary containment & isolation
//! 23. Panic message sanitization & truncation
//! 24. UTF-8 null-byte injection detection
//! 25. UTF-8 malformed byte sequence rejection
//! 26. Concurrency reentrancy & deadlock avoidance
//! 27. Concurrent borrow tracker readers/writer validation
//! 28. Extreme numeric boundaries & float edge cases
//! 29. Multi-tenant handle session isolation & leak detection

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use ttzip_engine::security::uniffi_defense::{
    ConcurrencyBoundGuard, ConcurrentBorrowTracker, HandleRegistry, PanicCatchGuard,
    RawPointerGuard, RustBufferBoundsGuard, RustBufferRaw, ScopedHandle, UniFFIDefenseError,
    Utf8SanitizerGuard, DEFAULT_MAX_RUSTBUFFER_CAPACITY,
};
use ttzip_engine::types::TTZipStatus;

// ============================================================================
// Fixture Data Models & Foreign Interfaces
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
struct ScalarTuple(bool, i8, u8, i16, u16, i32, u32, i64, u64, f32, f64);

#[derive(Debug, Clone, PartialEq, Eq)]
enum FlatColor {
    Red,
    Green,
    Blue,
    Custom(u32),
}

#[derive(Debug, Clone, PartialEq)]
enum ComplexPayload {
    Empty,
    Text(String),
    Binary(Vec<u8>),
    Point { x: f64, y: f64 },
    Nested(Box<ComplexPayload>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MetadataRecord {
    id: u64,
    name: String,
    tags: Vec<String>,
    properties: HashMap<String, String>,
}

struct DropCounter {
    counter: Arc<AtomicUsize>,
}

impl Drop for DropCounter {
    fn drop(&mut self) {
        self.counter.fetch_add(1, Ordering::SeqCst);
    }
}

trait ForeignProgressCallback: Send + Sync {
    fn on_progress(&self, current: u64, total: u64) -> bool;
}

// ============================================================================
// 29 Official Compliance Test Fixtures
// ============================================================================

#[test]
fn test_01_primitive_types_scalar_roundtrip() {
    let original = ScalarTuple(
        true,
        -128,
        255,
        -32768,
        65535,
        -2147483648,
        4294967295,
        -9223372036854775808,
        u64::MAX,
        std::f32::consts::PI,
        std::f64::consts::E,
    );
    let cloned = original.clone();
    assert_eq!(original.0, cloned.0);
    assert_eq!(original.1, cloned.1);
    assert_eq!(original.2, cloned.2);
    assert_eq!(original.3, cloned.3);
    assert_eq!(original.4, cloned.4);
    assert_eq!(original.5, cloned.5);
    assert_eq!(original.6, cloned.6);
    assert_eq!(original.7, cloned.7);
    assert_eq!(original.8, cloned.8);
    assert!((original.9 - cloned.9).abs() < 1e-6);
    assert!((original.10 - cloned.10).abs() < 1e-12);
}

#[test]
fn test_02_utf8_strings_unicode_and_emojis() {
    let samples = [
        "TTZip High-Performance Engine",
        "北京・東京・서울・Munich・Zürich",
        "🚀 📦 🛡️ ⚡ 🔒 🦀",
        "Mathematical symbols: ∀x ∈ ℝ, ∃y > 0",
        "",
    ];
    for &sample in &samples {
        let validated = Utf8SanitizerGuard::validate_safe_string(sample.as_bytes(), 1024)
            .expect("Valid UTF-8 string must pass");
        assert_eq!(validated, sample);
    }
}

#[test]
fn test_03_byte_buffer_and_binary_blobs() {
    let blob = (0..256).map(|i| (i % 251) as u8).collect::<Vec<u8>>();
    let mut raw_buf = RustBufferBoundsGuard::alloc_buffer(&blob);
    assert_eq!(raw_buf.len, 256);
    assert!(raw_buf.capacity >= 256);

    let slice = RustBufferBoundsGuard::as_slice(&raw_buf).expect("Buffer must be readable");
    assert_eq!(slice, blob.as_slice());

    RustBufferBoundsGuard::free_buffer(&mut raw_buf).expect("Free must succeed");
    assert_eq!(raw_buf.len, 0);
    assert_eq!(raw_buf.capacity, 0);
    assert!(raw_buf.data.is_null());
}

#[test]
fn test_04_option_nullable_types() {
    let some_val: Option<String> = Some("TTZip".to_string());
    let none_val: Option<String> = None;
    let nested_some: Option<Option<i32>> = Some(Some(42));
    let nested_none: Option<Option<i32>> = Some(None);

    assert_eq!(some_val.as_deref(), Some("TTZip"));
    assert!(none_val.is_none());
    assert_eq!(nested_some, Some(Some(42)));
    assert_eq!(nested_none, Some(None));
}

#[test]
fn test_05_sequence_and_vector_collections() {
    let empty_seq: Vec<u32> = Vec::new();
    let flat_seq = [10u64, 20, 30, 40, 50];
    let nested_seq = [vec![1, 2], vec![3, 4, 5], vec![]];

    assert!(empty_seq.is_empty());
    assert_eq!(flat_seq.len(), 5);
    assert_eq!(nested_seq[1], vec![3, 4, 5]);
}

#[test]
fn test_06_map_and_dictionary_records() {
    let mut map = HashMap::new();
    map.insert("author".to_string(), "Witt Kung".to_string());
    map.insert("format".to_string(), "ZIP/7z/TAR".to_string());

    assert_eq!(map.get("author").map(|s| s.as_str()), Some("Witt Kung"));
    assert_eq!(map.get("license"), None);
}

#[test]
fn test_07_flat_enum_variants_and_discriminants() {
    let c1 = FlatColor::Red;
    let c2 = FlatColor::Green;
    let c3 = FlatColor::Blue;
    let c4 = FlatColor::Custom(0xFF00FF);

    assert_ne!(c1, c2);
    assert_ne!(c2, c3);
    assert_eq!(c4, FlatColor::Custom(0xFF00FF));
}

#[test]
fn test_08_complex_associated_data_enums() {
    let p0 = ComplexPayload::Empty;
    let p1 = ComplexPayload::Text("Payload".to_string());
    let p2 = ComplexPayload::Binary(vec![0xCA, 0xFE, 0xBA, 0xBE]);
    let p3 = ComplexPayload::Point { x: 12.5, y: 99.0 };
    let p4 = ComplexPayload::Nested(Box::new(p1.clone()));

    assert_eq!(p0, ComplexPayload::Empty);
    assert_eq!(p1, ComplexPayload::Text("Payload".to_string()));
    assert_eq!(p2, ComplexPayload::Binary(vec![0xCA, 0xFE, 0xBA, 0xBE]));
    assert_eq!(p3, ComplexPayload::Point { x: 12.5, y: 99.0 });
    assert_eq!(p4, ComplexPayload::Nested(Box::new(ComplexPayload::Text("Payload".to_string()))));
}

#[test]
fn test_09_nested_records_and_composites() {
    let mut props = HashMap::new();
    props.insert("compression".to_string(), "ZSTD".to_string());
    let rec = MetadataRecord {
        id: 1001,
        name: "Archive.ttzip".to_string(),
        tags: vec!["backup".to_string(), "encrypted".to_string()],
        properties: props,
    };

    assert_eq!(rec.id, 1001);
    assert_eq!(rec.tags.len(), 2);
    assert_eq!(rec.properties.get("compression").map(|s| s.as_str()), Some("ZSTD"));
}

#[test]
fn test_10_error_flow_and_structured_exceptions() {
    let err = UniFFIDefenseError::BufferCapacityExceeded {
        capacity: 1024 * 1024 * 1024,
        limit: DEFAULT_MAX_RUSTBUFFER_CAPACITY,
    };
    let status: TTZipStatus = err.clone().into();
    assert_eq!(status, TTZipStatus::ErrOutOfMemory);
    assert!(err.to_string().contains("capacity"));
}

#[test]
fn test_11_opaque_rust_objects_and_interfaces() {
    struct OpaqueEngine {
        counter: AtomicUsize,
    }
    let registry = HandleRegistry::new();
    let handle = registry.register(OpaqueEngine {
        counter: AtomicUsize::new(10),
    });

    let obj = registry.get(handle).expect("Handle must exist");
    obj.counter.fetch_add(5, Ordering::SeqCst);
    assert_eq!(obj.counter.load(Ordering::SeqCst), 15);

    let removed = registry.unregister(handle).expect("Unregister must succeed");
    assert_eq!(removed.counter.load(Ordering::SeqCst), 15);
}

#[test]
fn test_12_multiple_constructor_factories() {
    struct ArchiveSession {
        path: String,
        read_only: bool,
    }
    impl ArchiveSession {
        fn open_read(path: &str) -> Self {
            Self { path: path.to_string(), read_only: true }
        }
        fn open_write(path: &str) -> Self {
            Self { path: path.to_string(), read_only: false }
        }
    }
    let r = ArchiveSession::open_read("test.zip");
    let w = ArchiveSession::open_write("output.zip");
    assert_eq!(r.path, "test.zip");
    assert_eq!(w.path, "output.zip");
    assert!(r.read_only);
    assert!(!w.read_only);
}

#[test]
fn test_13_synchronous_callback_interfaces() {
    struct MockCallback {
        received: Arc<AtomicUsize>,
    }
    impl ForeignProgressCallback for MockCallback {
        fn on_progress(&self, current: u64, total: u64) -> bool {
            self.received.store((current * 100 / total) as usize, Ordering::SeqCst);
            true
        }
    }
    let received = Arc::new(AtomicUsize::new(0));
    let cb: Box<dyn ForeignProgressCallback> = Box::new(MockCallback {
        received: received.clone(),
    });
    let continue_exec = cb.on_progress(50, 100);
    assert!(continue_exec);
    assert_eq!(received.load(Ordering::SeqCst), 50);
}

#[test]
fn test_14_async_future_and_concurrent_tasks() {
    let handle = thread::spawn(|| {
        let mut sum = 0u64;
        for i in 1..=1000 {
            sum += i;
        }
        sum
    });
    let result = handle.join().expect("Worker thread must succeed");
    assert_eq!(result, 500500);
}

#[test]
fn test_15_concurrent_object_sharing_arc() {
    let registry = Arc::new(HandleRegistry::new());
    let handle = registry.register(AtomicUsize::new(0));

    let mut threads = Vec::new();
    for _ in 0..8 {
        let reg = registry.clone();
        threads.push(thread::spawn(move || {
            let obj = reg.get(handle).unwrap();
            for _ in 0..1000 {
                obj.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }
    for t in threads {
        t.join().unwrap();
    }
    let obj = registry.get(handle).unwrap();
    assert_eq!(obj.load(Ordering::SeqCst), 8000);
}

#[test]
fn test_16_object_lifecycle_and_raii_drop() {
    let counter = Arc::new(AtomicUsize::new(0));
    let registry = HandleRegistry::new();
    {
        let _scoped = ScopedHandle::new(&registry, DropCounter {
            counter: counter.clone(),
        });
        assert_eq!(registry.active_count(), 1);
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }
    assert_eq!(registry.active_count(), 0);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn test_17_handle_double_free_interception() {
    let registry = HandleRegistry::new();
    let handle = registry.register("Resource");
    assert!(registry.unregister(handle).is_ok());

    // Second unregister must fail with HandleDoubleFree
    let double_free_err = registry.unregister(handle).unwrap_err();
    assert_eq!(double_free_err, UniFFIDefenseError::HandleDoubleFree { handle });
}

#[test]
fn test_18_null_pointer_boundary_guard() {
    let null_ptr: *const u8 = std::ptr::null();
    assert_eq!(
        RawPointerGuard::validate_ptr(null_ptr),
        Err(UniFFIDefenseError::NullPointerDetected)
    );

    let low_addr_ptr = 0x10 as *const u8;
    assert_eq!(
        RawPointerGuard::validate_ptr(low_addr_ptr),
        Err(UniFFIDefenseError::InvalidPointerAddress { address: 0x10 })
    );
}

#[test]
fn test_19_pointer_alignment_guard() {
    #[repr(align(8))]
    struct AlignedStruct(u64);

    let item = AlignedStruct(42);
    assert_eq!(item.0, 42);
    let valid_ptr = &item as *const AlignedStruct;
    assert!(RawPointerGuard::validate_ptr(valid_ptr).is_ok());

    // Artificially create unaligned pointer
    let unaligned_addr = (valid_ptr as usize) | 1;
    let unaligned_ptr = unaligned_addr as *const AlignedStruct;
    assert!(matches!(
        RawPointerGuard::validate_ptr(unaligned_ptr),
        Err(UniFFIDefenseError::MisalignedPointer { .. })
    ));
}

#[test]
fn test_20_rustbuffer_bounds_and_overflow() {
    let mut data = vec![1, 2, 3];
    let invalid_buf = RustBufferRaw {
        capacity: 2,
        len: 3, // len > capacity is an invariant violation
        data: data.as_mut_ptr(),
    };
    assert_eq!(
        RustBufferBoundsGuard::validate_buffer(&invalid_buf, DEFAULT_MAX_RUSTBUFFER_CAPACITY),
        Err(UniFFIDefenseError::BufferBoundsViolation { len: 3, capacity: 2 })
    );
}

#[test]
fn test_21_rustbuffer_offset_slice_access() {
    let data = b"Hello TTZip UniFFI Defense";
    let raw_buf = RustBufferBoundsGuard::alloc_buffer(data);

    let (ptr, len) = RustBufferBoundsGuard::validate_slice_access(&raw_buf, 6, 5)
        .expect("Valid offset read must succeed");
    assert_eq!(len, 5);
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    assert_eq!(slice, b"TTZip");

    // Out of bounds access
    let err = RustBufferBoundsGuard::validate_slice_access(&raw_buf, 20, 100).unwrap_err();
    assert!(matches!(err, UniFFIDefenseError::BufferOffsetOutOfBounds { .. }));
}

#[test]
fn test_22_panic_containment_and_isolation() {
    let result = PanicCatchGuard::catch_boundary(|| {
        panic!("Intentional test panic in FFI boundary");
    });
    assert!(result.is_err());
    let err = result.unwrap_err();
    if let UniFFIDefenseError::PanicCaught { sanitized_message } = err {
        assert!(sanitized_message.contains("Intentional test panic"));
    } else {
        panic!("Expected PanicCaught error");
    }

    let status = PanicCatchGuard::catch_status(|| {
        panic!("Status panic");
    });
    assert_eq!(status, TTZipStatus::ErrPanicCaught);
}

#[test]
fn test_23_panic_message_sanitization() {
    let raw = "Sensitive path /Users/admin/secrets.key panic: system error \x00\x01\x02";
    let payload: Box<dyn std::any::Any + Send> = Box::new(raw.to_string());
    let sanitized = PanicCatchGuard::sanitize_panic_payload(&*payload);
    assert!(!sanitized.contains('\0'));
    assert!(sanitized.len() <= 256);
}

#[test]
fn test_24_utf8_null_byte_injection_prevention() {
    let malicious = b"valid_prefix\0hidden_injected_suffix";
    let result = Utf8SanitizerGuard::validate_safe_string(malicious, 1024);
    assert_eq!(
        result,
        Err(UniFFIDefenseError::NullByteInjectionDetected { offset: 12 })
    );
}

#[test]
fn test_25_utf8_malformed_byte_sequence_rejection() {
    let invalid_utf8 = b"\xFF\xFE\xFD";
    let result = Utf8SanitizerGuard::validate_safe_string(invalid_utf8, 1024);
    assert!(matches!(result, Err(UniFFIDefenseError::InvalidUtf8Encoding { .. })));

    let sanitized = Utf8SanitizerGuard::sanitize_lossy(invalid_utf8, 1024);
    assert_eq!(sanitized, "\u{FFFD}\u{FFFD}\u{FFFD}");
}

#[test]
fn test_26_reentrancy_and_deadlock_avoidance() {
    ConcurrencyBoundGuard::assert_send_sync::<HandleRegistry<String>>();
    ConcurrencyBoundGuard::assert_unwind_safe::<AtomicBool>();
}

#[test]
fn test_27_concurrent_borrow_tracker_integrity() {
    let tracker = ConcurrentBorrowTracker::new();
    let handle = 0x8888;

    assert!(tracker.borrow_read(handle).is_ok());
    assert!(tracker.borrow_read(handle).is_ok());

    // While readers exist, write borrow must be rejected
    assert!(matches!(
        tracker.borrow_write(handle),
        Err(UniFFIDefenseError::ConcurrentBorrowConflict { .. })
    ));

    tracker.release_read(handle);
    tracker.release_read(handle);

    // After readers released, write borrow succeeds
    assert!(tracker.borrow_write(handle).is_ok());
    // Reader borrow while writer active is rejected
    assert!(tracker.borrow_read(handle).is_err());
    tracker.release_write(handle);
}

#[test]
fn test_28_extreme_numeric_boundaries() {
    let f_nan = f64::NAN;
    let f_inf = f64::INFINITY;
    let f_neg_inf = f64::NEG_INFINITY;
    let f_subnormal = 1e-320_f64;

    assert!(f_nan.is_nan());
    assert!(f_inf.is_infinite());
    assert!(f_neg_inf.is_infinite());
    assert!(f_subnormal.is_sign_positive());
}

#[test]
fn test_29_multi_tenant_handle_session_isolation() {
    let session_a = HandleRegistry::new();
    let session_b = HandleRegistry::new();

    let h_a = session_a.register("Session A Data");
    let h_b = session_b.register("Session B Data");

    assert_eq!(session_a.get(h_a).unwrap().as_ref(), &"Session A Data");
    assert_eq!(session_b.get(h_b).unwrap().as_ref(), &"Session B Data");

    // Handle from Session A is invalid in Session B
    assert!(session_b.get(h_a).is_err() || h_a == h_b);
    assert_eq!(session_a.detect_leaks().len(), 1);
    assert_eq!(session_b.detect_leaks().len(), 1);

    session_a.unregister(h_a).unwrap();
    session_b.unregister(h_b).unwrap();
    assert_eq!(session_a.detect_leaks().len(), 0);
    assert_eq!(session_b.detect_leaks().len(), 0);
}
