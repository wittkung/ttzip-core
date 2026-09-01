// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unit tests for PDF 6-Layer Defense-in-Depth guards.

use std::io::Write;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use lopdf::dictionary;
use sha2::{Digest, Sha256};

use super::encryption::PDF_STANDARD_PASSWORD_PADDING;
use super::*;

#[test]
fn test_cycle_guard_direct_and_indirect_loops() {
    let mut guard = IndirectReferenceCycleGuard::with_limits(10, 100);

    // 1. Direct self-reference cycle
    {
        let scope1 = guard.enter_object((1, 0)).expect("Enter obj 1");
        let res = guard.enter_object((1, 0));
        assert!(matches!(res, Err(PdfDefenseError::CycleDetected { .. })));
        guard.leave_scope(scope1);
    }

    // 2. 3-hop indirect cycle (1 -> 2 -> 3 -> 1)
    guard.reset();
    {
        let s1 = guard.enter_object((1, 0)).unwrap();
        let s2 = guard.enter_object((2, 0)).unwrap();
        let s3 = guard.enter_object((3, 0)).unwrap();
        let res = guard.enter_object((1, 0));
        assert!(matches!(res, Err(PdfDefenseError::CycleDetected { .. })));
        guard.leave_scope(s3);
        guard.leave_scope(s2);
        guard.leave_scope(s1);
    }

    // 3. Normal tree structure after exit
    guard.reset();
    {
        {
            let s1 = guard.enter_object((1, 0)).unwrap();
            let s2 = guard.enter_object((2, 0)).unwrap();
            guard.leave_scope(s2);
            guard.leave_scope(s1);
        }
        // Exited 2, re-entering 2 from another parent should succeed
        let s3 = guard.enter_object((3, 0)).unwrap();
        let s2_again = guard.enter_object((2, 0)).unwrap();
        guard.leave_scope(s2_again);
        guard.leave_scope(s3);
    }
}

#[test]
fn test_cycle_guard_depth_and_count_limits() {
    // Test depth limit
    let mut guard = IndirectReferenceCycleGuard::with_limits(3, 100);
    let s1 = guard.enter_object((1, 0)).unwrap();
    let s2 = guard.enter_object((2, 0)).unwrap();
    let s3 = guard.enter_object((3, 0)).unwrap();
    let res = guard.enter_object((4, 0));
    assert!(matches!(
        res,
        Err(PdfDefenseError::MaxRecursionDepthExceeded { depth: 4, max_depth: 3 })
    ));
    guard.leave_scope(s3);
    guard.leave_scope(s2);
    guard.leave_scope(s1);

    // Test visit count limit
    let mut guard = IndirectReferenceCycleGuard::with_limits(10, 3);
    {
        let s1 = guard.enter_object((1, 0)).unwrap();
        guard.leave_scope(s1);
    }
    {
        let s2 = guard.enter_object((2, 0)).unwrap();
        guard.leave_scope(s2);
    }
    {
        let s3 = guard.enter_object((3, 0)).unwrap();
        guard.leave_scope(s3);
    }
    let res = guard.enter_object((4, 0));
    assert!(matches!(
        res,
        Err(PdfDefenseError::ObjectCountExceeded { count: 4, max_count: 3 })
    ));
}

#[test]
fn test_stream_expansion_guard_decompression_bomb() {
    let mut guard = StreamExpansionQuotaGuard::with_limits(1024 * 1024, 50.0, 10 * 1024 * 1024);

    // Highly compressible payload: 50,000 zeros compressed to ~50 bytes (~1000x expansion)
    let raw_payload = vec![0u8; 50_000];
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&raw_payload).unwrap();
    let compressed = encoder.finish().unwrap();

    let res = guard.decompress_flate(&compressed);
    assert!(matches!(
        res,
        Err(PdfDefenseError::StreamExpansionRatioExceeded { .. })
    ));

    // Single stream size limit
    let mut guard_small = StreamExpansionQuotaGuard::with_limits(100, 1000.0, 10000);
    let res2 = guard_small.decompress_flate(&compressed);
    assert!(matches!(res2, Err(PdfDefenseError::StreamSizeExceeded { .. })));

    // Legitimate small stream
    let normal_payload = b"Hello world, normal stream content for testing.";
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(normal_payload).unwrap();
    let comp_normal = enc.finish().unwrap();

    let mut guard_ok = StreamExpansionQuotaGuard::new();
    let decomp = guard_ok.decompress_flate(&comp_normal).unwrap();
    assert_eq!(decomp.as_slice(), normal_payload);
}

#[test]
fn test_page_tree_guard_depth_and_cycle() {
    let mut doc = lopdf::Document::new();

    // Page (1,0)
    let page1_id = doc.add_object(lopdf::dictionary! {
        "Type" => "Page",
    });

    // Pages branch (2,0) containing Page (1,0)
    let pages2_id = doc.add_object(lopdf::dictionary! {
        "Type" => "Pages",
        "Kids" => vec![lopdf::Object::Reference(page1_id)],
        "Count" => 1,
    });

    // Root Pages (3,0) containing branch (2,0)
    let root_pages_id = doc.add_object(lopdf::dictionary! {
        "Type" => "Pages",
        "Kids" => vec![lopdf::Object::Reference(pages2_id)],
        "Count" => 1,
    });

    // Catalog -> Pages (3,0)
    let catalog_id = doc.add_object(lopdf::dictionary! {
        "Type" => "Catalog",
        "Pages" => lopdf::Object::Reference(root_pages_id),
    });
    doc.trailer.set("Root", lopdf::Object::Reference(catalog_id));

    let guard = PageTreeDepthGuard::new();
    let res = guard.collect_pages_iterative(&doc).unwrap();
    assert_eq!(res.page_count, 1);
    assert_eq!(res.leaf_page_ids, vec![(page1_id.0, page1_id.1)]);

    // Depth limit breach
    let shallow_guard = PageTreeDepthGuard::with_limits(1, 100);
    let err = shallow_guard.collect_pages_iterative(&doc);
    assert!(matches!(err, Err(PdfDefenseError::PageTreeDepthExceeded { .. })));
}

#[test]
fn test_malicious_action_sandbox_guard() {
    let mut doc = lopdf::Document::new();

    let script_obj = doc.add_object(lopdf::dictionary! {
        "S" => "JavaScript",
        "JS" => lopdf::Object::String(b"app.alert('PWNED');".to_vec(), lopdf::StringFormat::Literal),
    });

    let catalog_id = doc.add_object(lopdf::dictionary! {
        "Type" => "Catalog",
        "OpenAction" => lopdf::Object::Reference(script_obj),
    });
    doc.trailer.set("Root", lopdf::Object::Reference(catalog_id));

    // Strict reject policy
    let reject_guard = MaliciousActionSandboxGuard::new(ActionPolicy::RejectAllActiveContent);
    let res = reject_guard.inspect_document(&doc);
    assert!(matches!(res, Err(PdfDefenseError::MaliciousActionDetected { .. })));

    // Dangerous URI schemes
    assert!(MaliciousActionSandboxGuard::is_dangerous_uri("file:///etc/passwd"));
    assert!(MaliciousActionSandboxGuard::is_dangerous_uri("javascript:alert(1)"));
    assert!(MaliciousActionSandboxGuard::is_dangerous_uri("powershell:iex"));
    assert!(!MaliciousActionSandboxGuard::is_dangerous_uri("https://example.com/doc.pdf"));

    // Sanitize policy
    let sanitize_guard = MaliciousActionSandboxGuard::new(ActionPolicy::SanitizeAndStrip);
    let mut doc_clone = doc.clone();
    let report = sanitize_guard.sanitize_document(&mut doc_clone).unwrap();
    assert!(report.is_sanitized);

    // After sanitization, inspection must pass cleanly
    let audit_guard = MaliciousActionSandboxGuard::new(ActionPolicy::RejectAllActiveContent);
    assert!(audit_guard.inspect_document(&doc_clone).is_ok());
}

#[test]
fn test_pdf_encryption_guard_and_downgrade_defense() {
    let mut doc = lopdf::Document::new();

    // 40-bit RC4 insecure encrypt dict
    let enc_id = doc.add_object(lopdf::dictionary! {
        "Filter" => "Standard",
        "V" => 1,
        "R" => 2,
        "Length" => 40,
        "P" => -64,
        "O" => lopdf::Object::String(vec![0u8; 32], lopdf::StringFormat::Hexadecimal),
        "U" => lopdf::Object::String(vec![0u8; 32], lopdf::StringFormat::Hexadecimal),
    });
    doc.trailer.set("Encrypt", lopdf::Object::Reference(enc_id));

    // Enforce modern AES policy blocks RC4-40
    let guard_modern = PdfEncryptionGuard::new(EncryptionSecurityPolicy::EnforceModernAesOnly);
    let res = guard_modern.inspect_document(&doc);
    assert!(matches!(res, Err(PdfDefenseError::InsecureEncryptionDetected { .. })));

    // Allow policy inspects without error
    let guard_allow = PdfEncryptionGuard::new(EncryptionSecurityPolicy::AllowStandardAndModern);
    let rep = guard_allow.inspect_document(&doc).unwrap();
    assert_eq!(rep.cipher_suite, CipherSuite::Rc4_40);
    assert_eq!(rep.key_length_bits, 40);

    // Constant-time password probe
    let expected = Sha256::digest(PDF_STANDARD_PASSWORD_PADDING);
    assert!(guard_allow.verify_password_probe(b"", &expected));
}

#[test]
fn test_sensitive_pdf_buffer_zeroize() {
    let secret = b"super_secret_pdf_content";
    let mut buf = SensitivePdfBuffer::from_slice(secret);
    assert_eq!(&*buf, secret);
    assert!(buf.constant_time_eq(secret));
    assert!(!buf.constant_time_eq(b"wrong_secret"));

    buf.clear_and_zeroize();
    assert!(buf.is_empty());
}
