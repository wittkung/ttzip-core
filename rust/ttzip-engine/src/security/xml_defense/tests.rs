// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use super::*;

#[test]
fn test_xxe_external_entity_detection() {
    let bad_xml = br#"<?xml version="1.0"?><!DOCTYPE foo [<!ENTITY xxe SYSTEM "file:///etc/passwd">]><foo>&xxe;</foo>"#;
    assert!(XxeExternalEntityGuard::scan_for_xxe(bad_xml).is_err());

    let safe_xml = br#"<?xml version="1.0"?><w:document xmlns:w="http://main"><w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body></w:document>"#;
    assert!(XxeExternalEntityGuard::scan_for_xxe(safe_xml).is_ok());
}

#[test]
fn test_max_depth_guard() {
    let mut guard = MaxDepthGuard::with_max_depth(3);
    assert!(guard.push_element("a").is_ok());
    assert!(guard.push_element("b").is_ok());
    assert!(guard.push_element("c").is_ok());
    assert!(guard.push_element("d").is_err());
}

#[test]
fn test_stream_recovery() {
    let broken = "<w:document><w:body><w:p><w:r><w:t>Incomplete";
    let healed = MalformedStreamRecoveryGuard::heal_truncated_stream(broken);
    assert!(healed.ends_with("</w:t></w:r></w:p></w:body></w:document>"));
}

#[test]
fn test_sensitive_buffer_zeroize() {
    let mut buf = SensitiveXmlBuffer::from_string("SECRET_XML_CREDENTIAL".to_string());
    assert_eq!(buf.len(), 21);
    buf.clear_and_zeroize();
    assert_eq!(buf.len(), 0);
}
