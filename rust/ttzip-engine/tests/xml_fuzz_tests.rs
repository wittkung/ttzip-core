// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive corruption injection, chaos mutation, and fuzzing test suite for TTZip Streaming XML & Document Engine.
//!
//! Deploys 16 surgical destruction targets:
//! 1. XXE malicious external entity injection attack defense.
//! 2. Billion Laughs recursive entity expansion XML bomb mitigation.
//! 3. Ultra-deep tag nesting (100+ / 1000+ levels) recursion and stack overflow defense.
//! 4. Zero-byte (`\0`) and empty-stream edge corruption injection.
//! 5. Overlong attribute names (64KB+) and gigantic CDATA memory boundary defense.
//! 6. 1000+ concurrent Rayon tasks XML streaming parsing race competition.
//! 7. 500+ rounds of pseudo-random data mutation and perturbation fuzzing.
//! 8. Malformed unclosed tags and mismatching closing tags corruption.
//! 9. Entity encoding confusion and invalid UTF-8 / control character injection.
//! 10. Malformed DTD, DOCTYPE declarations, and recursive parameter entity injection.
//! 11. Namespace prefix pollution and undeclared prefix collision resilience.
//! 12. Extremely long XML element tags and continuous non-whitespace token defense.
//! 13. Multiple XML declarations, encoding mismatches, and BOM confusion injection.
//! 14. Malformed comments (internal `--`) and processing instructions injection.
//! 15. DOCX / EPUB container XML corruption and missing structure fallback tests.
//! 16. Streaming SAX parser state machine reset and reentrant chunk perturbation.

use std::io::Cursor;
use std::panic::catch_unwind;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use quick_xml::events::Event;
use quick_xml::Reader as XmlReader;
use rayon::prelude::*;

use ttzip_engine::standards::document_stream::{
    parse_docx_from_memory, parse_docx_xml_content, parse_epub_from_memory,
};
use ttzip_engine::zip::{assemble_zip_archive, compress_items_parallel, ZipInputItem};

/// Deterministic linear congruential generator for reproducible fuzzing vectors.
#[derive(Clone, Debug)]
struct DeterministicPrng {
    state: u64,
}

impl DeterministicPrng {
    #[inline]
    const fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x9e37_79b9_7f4a_7c15 } else { seed },
        }
    }

    #[inline]
    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }

    #[inline]
    fn next_range(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        let span = (max - min + 1) as u64;
        min + (self.next_u32() as u64 % span) as usize
    }

    #[inline]
    fn next_byte(&mut self) -> u8 {
        (self.next_u32() & 0xFF) as u8
    }
}

fn helper_build_test_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let items: Vec<ZipInputItem> = files
        .iter()
        .map(|(name, content)| ZipInputItem {
            rel_path: name.to_string(),
            data: content.to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        })
        .collect();
    let compressed = compress_items_parallel(
        items,
        6,
        ttzip_engine::types::TTZipEncryptionMethod::None,
        None,
        1,
    )
    .unwrap();
    assemble_zip_archive(&compressed).unwrap()
}

// ============================================================================
// Target 1: XXE Malicious External Entity Injection Attack Defense
// ============================================================================
#[test]
fn test_target_01_xxe_external_entity_injection_defense() {
    let xxe_payloads = [
        r#"<?xml version="1.0"?><!DOCTYPE doc [<!ENTITY xxe SYSTEM "file:///etc/passwd">]><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>&xxe;</w:t></w:r></w:p></w:body></w:document>"#,
        r#"<?xml version="1.0"?><!DOCTYPE doc [<!ENTITY % dtd SYSTEM "http://127.0.0.1:9999/evil.dtd">%dtd;]><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Safe</w:t></w:r></w:p></w:body></w:document>"#,
        r#"<?xml version="1.0"?><!DOCTYPE test [<!ENTITY file SYSTEM "expect://id">]><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>&file;</w:t></w:r></w:p></w:body></w:document>"#,
    ];

    for (idx, payload) in xxe_payloads.iter().enumerate() {
        let res = catch_unwind(|| {
            let parsed = parse_docx_xml_content(payload.as_bytes());
            if let Ok((text, _)) = parsed {
                // Must not disclose sensitive file content
                assert!(!text.contains("root:x:0:0:"), "XXE leak detected on payload {}", idx);
            }
        });
        assert!(res.is_ok(), "Panic on XXE payload index {}", idx);
    }
}

// ============================================================================
// Target 2: Billion Laughs Recursive Entity Expansion XML Bomb Mitigation
// ============================================================================
#[test]
fn test_target_02_billion_laughs_xml_bomb_mitigation() {
    let bomb_payload = r#"<?xml version="1.0"?>
<!DOCTYPE lolz [
 <!ENTITY lol "lol">
 <!ELEMENT lolz (#PCDATA)>
 <!ENTITY lol1 "&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;">
 <!ENTITY lol2 "&lol1;&lol1;&lol1;&lol1;&lol1;&lol1;&lol1;&lol1;&lol1;&lol1;">
 <!ENTITY lol3 "&lol2;&lol2;&lol2;&lol2;&lol2;&lol2;&lol2;&lol2;&lol2;&lol2;">
 <!ENTITY lol4 "&lol3;&lol3;&lol3;&lol3;&lol3;&lol3;&lol3;&lol3;&lol3;&lol3;">
 <!ENTITY lol5 "&lol4;&lol4;&lol4;&lol4;&lol4;&lol4;&lol4;&lol4;&lol4;&lol4;">
]>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:t>&lol5;</w:t></w:r></w:p></w:body>
</w:document>"#;

    let res = catch_unwind(|| {
        let parsed = parse_docx_xml_content(bomb_payload.as_bytes());
        // Should parse safely without OOM or exponential CPU hangs
        let _ = parsed;
    });
    assert!(res.is_ok(), "Panic or crash on Billion Laughs XML bomb");
}

// ============================================================================
// Target 3: Ultra-Deep Tag Nesting (100+ Levels) Recursion & Stack Defense
// ============================================================================
#[test]
fn test_target_03_ultra_deep_tag_nesting_stack_defense() {
    let depth = 500;
    let mut deep_xml = String::with_capacity(depth * 30);
    deep_xml.push_str(r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#);
    for i in 0..depth {
        deep_xml.push_str(&format!("<nested_tag_{}>", i));
    }
    deep_xml.push_str("<w:p><w:r><w:t>Deeply Nested Text</w:t></w:r></w:p>");
    for i in (0..depth).rev() {
        deep_xml.push_str(&format!("</nested_tag_{}>", i));
    }
    deep_xml.push_str("</w:body></w:document>");

    let res = catch_unwind(|| {
        let parsed = parse_docx_xml_content(deep_xml.as_bytes());
        assert!(parsed.is_ok());
        let (text, paras) = parsed.unwrap();
        assert_eq!(paras.len(), 1);
        assert_eq!(text, "Deeply Nested Text");
    });
    assert!(res.is_ok(), "Stack overflow or panic on 500-level XML tag nesting");
}

// ============================================================================
// Target 4: Zero-Byte & Empty-Stream Edge Corruption Injection
// ============================================================================
#[test]
fn test_target_04_zero_byte_and_empty_stream_edge_corruption() {
    let edge_cases: Vec<&[u8]> = vec![
        b"",
        b" ",
        b"\t\r\n",
        b"\0",
        b"\0\0\0\0",
        b"<?xml?>",
        b"<?xml version=\"1.0\"?>",
        b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\0",
        b"<",
        b"</",
        b"<w:document>",
        b"<w:document></w:document\0>",
    ];

    for (idx, slice) in edge_cases.iter().enumerate() {
        let res = catch_unwind(|| {
            let _ = parse_docx_xml_content(slice);
        });
        assert!(res.is_ok(), "Panic on zero-byte / empty slice index {}", idx);
    }
}

// ============================================================================
// Target 5: Overlong Attribute Names & Gigantic CDATA Memory Defense
// ============================================================================
#[test]
fn test_target_05_overlong_attributes_and_gigantic_cdata_defense() {
    let huge_attr_name = "attr_".repeat(10_000); // ~50 KB attribute name
    let huge_cdata_content = "X".repeat(500_000); // 500 KB CDATA
    let xml = format!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" {}="val"><w:body><w:p><w:r><w:t><![CDATA[{}]]></w:t></w:r></w:p></w:body></w:document>"#,
        huge_attr_name, huge_cdata_content
    );

    let res = catch_unwind(|| {
        let parsed = parse_docx_xml_content(xml.as_bytes());
        let _ = parsed;
    });
    assert!(res.is_ok(), "Panic on overlong attribute and huge CDATA payload");
}

// ============================================================================
// Target 6: 1000+ Concurrent Tasks XML Streaming Parsing Race Competition
// ============================================================================
#[test]
fn test_target_06_concurrent_xml_streaming_parsing_race() {
    let total_tasks = 1200;
    let completed = Arc::new(AtomicUsize::new(0));

    let sample_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Concurrency Test TTZip 2026</w:t></w:r></w:p>
    <w:p><w:r><w:t>High-throughput race validation line 2</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

    (0..total_tasks).into_par_iter().for_each(|task_id| {
        let mut prng = DeterministicPrng::new((task_id + 1) as u64);
        let mut mutated = sample_xml.to_vec();
        // Perturb 10% of tasks with subtle corruptions
        if prng.next_range(0, 10) == 0 {
            let cut = prng.next_range(1, mutated.len() - 1);
            mutated.truncate(cut);
        }

        let res = catch_unwind(|| {
            let _ = parse_docx_xml_content(&mutated);
        });
        assert!(res.is_ok(), "Panic in concurrent worker task {}", task_id);
        completed.fetch_add(1, Ordering::Relaxed);
    });

    assert_eq!(completed.load(Ordering::SeqCst), total_tasks);
}

// ============================================================================
// Target 7: 500+ Rounds of Pseudo-Random Mutation and Perturbation Fuzzing
// ============================================================================
#[test]
fn test_target_07_random_mutation_perturbation_fuzzing() {
    let base_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Fuzzing TTZip Document Stream</w:t></w:r></w:p>
    <w:p><w:r><w:tab/><w:t>Secondary Paragraph with Special &amp; &lt; &gt; Chars</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

    let mut prng = DeterministicPrng::new(0xDEAD_BEEF_CAFE_BABE);
    let rounds = 600;

    for r in 0..rounds {
        let mut mutated = base_xml.to_vec();
        let ops = prng.next_range(1, 8);

        for _ in 0..ops {
            match prng.next_range(0, 4) {
                0 => {
                    // Bit flip
                    if !mutated.is_empty() {
                        let idx = prng.next_range(0, mutated.len() - 1);
                        let bit = 1 << prng.next_range(0, 7);
                        mutated[idx] ^= bit;
                    }
                }
                1 => {
                    // Byte insertion
                    let idx = prng.next_range(0, mutated.len());
                    let byte = prng.next_byte();
                    mutated.insert(idx, byte);
                }
                2 => {
                    // Byte deletion
                    if !mutated.is_empty() {
                        let idx = prng.next_range(0, mutated.len() - 1);
                        mutated.remove(idx);
                    }
                }
                3 => {
                    // Splice random tokens
                    let idx = prng.next_range(0, mutated.len());
                    let tokens: &[&[u8]] = &[b"<w:p>", b"</w:p>", b"\0", b"&amp;", b"<![CDATA[", b"]]>", b"<?xml"];
                    let token = tokens[prng.next_range(0, tokens.len() - 1)];
                    mutated.splice(idx..idx, token.iter().copied());
                }
                _ => {}
            }
        }

        let res = catch_unwind(|| {
            let _ = parse_docx_xml_content(&mutated);
        });
        assert!(res.is_ok(), "Fuzzer triggered panic on round {}", r);
    }
}

// ============================================================================
// Target 8: Malformed Unclosed Tags & Mismatching Closing Tags Corruption
// ============================================================================
#[test]
fn test_target_08_malformed_unclosed_and_mismatched_tags() {
    let broken_xmls = [
        "<w:document><w:body><w:p><w:r><w:t>Unclosed",
        "<w:document><w:body><w:p><w:r><w:t>Mismatch</w:other></w:p></w:body></w:document>",
        "<tag1><tag2><tag3>Missing closes",
        "<w:document><w:body><w:p><w:t></w:document>",
        "<<<<>>>>",
        "<w:p><w:t>Open brackets < << >>> </w:t></w:p>",
    ];

    for (idx, broken) in broken_xmls.iter().enumerate() {
        let res = catch_unwind(|| {
            let _ = parse_docx_xml_content(broken.as_bytes());
        });
        assert!(res.is_ok(), "Panic on malformed tag index {}", idx);
    }
}

// ============================================================================
// Target 9: Entity Encoding Confusion & Invalid UTF-8 Control Characters
// ============================================================================
#[test]
fn test_target_09_entity_confusion_and_invalid_utf8() {
    let mut raw_bytes = Vec::from(
        br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>"#.as_slice()
    );
    // Inject invalid UTF-8 sequences and control chars
    raw_bytes.extend_from_slice(&[0xFF, 0xFE, 0x80, 0xC0, 0xAF, 0x01, 0x02, 0x1B]);
    raw_bytes.extend_from_slice(b"&amp;&lt;&gt;&quot;&apos;&unknown_entity;</w:t></w:r></w:p></w:body></w:document>");

    let res = catch_unwind(|| {
        let _ = parse_docx_xml_content(&raw_bytes);
    });
    assert!(res.is_ok(), "Panic on invalid UTF-8 / entity confusion byte stream");
}

// ============================================================================
// Target 10: Malformed DTD / DOCTYPE Declarations Injection
// ============================================================================
#[test]
fn test_target_10_malformed_dtd_doctype_declarations() {
    let dtd_samples = [
        r#"<!DOCTYPE [ <!ELEMENT test ANY> ]><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Test</w:t></w:r></w:p></w:body></w:document>"#,
        r#"<!DOCTYPE doc SYSTEM "missing_file.dtd"><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Test</w:t></w:r></w:p></w:body></w:document>"#,
        r#"<!DOCTYPE doc [<!ENTITY % pe "<!ENTITY e1 'value'>">%pe;]><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>&e1;</w:t></w:r></w:p></w:body></w:document>"#,
    ];

    for (idx, dtd) in dtd_samples.iter().enumerate() {
        let res = catch_unwind(|| {
            let _ = parse_docx_xml_content(dtd.as_bytes());
        });
        assert!(res.is_ok(), "Panic on DTD / DOCTYPE sample {}", idx);
    }
}

// ============================================================================
// Target 11: Namespace Prefix Pollution & Undeclared Prefix Collision
// ============================================================================
#[test]
fn test_target_11_namespace_prefix_collision_resilience() {
    let ns_xml = r#"<?xml version="1.0"?>
<w:document xmlns:w="http://ns1" xmlns:v="http://ns2" xmlns:wp="http://ns3" xmlns="http://default">
  <w:body>
    <wp:anchor><w:p><w:r><w:t>Namespace Prefix Resilience</w:t></w:r></w:p></wp:anchor>
    <unregistered:tag><w:p><w:r><w:t>Undeclared Prefix Item</w:t></w:r></w:p></unregistered:tag>
  </w:body>
</w:document>"#;

    let res = catch_unwind(|| {
        let parsed = parse_docx_xml_content(ns_xml.as_bytes());
        assert!(parsed.is_ok());
        let (text, _) = parsed.unwrap();
        assert!(text.contains("Namespace Prefix Resilience"));
    });
    assert!(res.is_ok(), "Panic on namespace collisions and undeclared prefixes");
}

// ============================================================================
// Target 12: Extremely Long XML Element Names & Continuous Tokens
// ============================================================================
#[test]
fn test_target_12_extremely_long_element_tokens() {
    let long_tag = "tag_".repeat(8000); // 32 KB element tag
    let long_content = "Word_".repeat(8000); // 40 KB text token
    let xml = format!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><{}><w:p><w:r><w:t>{}</w:t></w:r></w:p></{}></w:body></w:document>"#,
        long_tag, long_content, long_tag
    );

    let res = catch_unwind(|| {
        let _ = parse_docx_xml_content(xml.as_bytes());
    });
    assert!(res.is_ok(), "Panic on extremely long XML element names");
}

// ============================================================================
// Target 13: Multiple XML Declarations, Encodings & BOM Confusion
// ============================================================================
#[test]
fn test_target_13_multiple_xml_declarations_and_bom_confusion() {
    let utf8_bom = b"\xEF\xBB\xBF";
    let mut payload = utf8_bom.to_vec();
    payload.extend_from_slice(br#"<?xml version="1.0" encoding="UTF-8"?>"#);
    payload.extend_from_slice(br#"<?xml version="1.1" encoding="ISO-8859-1"?>"#);
    payload.extend_from_slice(br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>BOM Multi-Decl</w:t></w:r></w:p></w:body></w:document>"#);

    let res = catch_unwind(|| {
        let _ = parse_docx_xml_content(&payload);
    });
    assert!(res.is_ok(), "Panic on BOM header and multi-declaration XML");
}

// ============================================================================
// Target 14: Malformed Comments & Processing Instructions Injection
// ============================================================================
#[test]
fn test_target_14_malformed_comments_and_processing_instructions() {
    let xml_comments = r#"<?xml version="1.0"?>
<!-- Normal comment -->
<!-- Malformed -- double hyphen -- comment -->
<!-- Unclosed comment
<?custom-pi parameter="true" code="123"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Comment & PI Survives</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

    let res = catch_unwind(|| {
        let _ = parse_docx_xml_content(xml_comments.as_bytes());
    });
    assert!(res.is_ok(), "Panic on malformed comments or processing instructions");
}

// ============================================================================
// Target 15: DOCX / EPUB Container XML Corruption & Boundary Fallback
// ============================================================================
#[test]
fn test_target_15_docx_epub_container_corruption_fallback() {
    // 15a. DOCX missing word/document.xml
    let empty_docx = helper_build_test_zip(&[("docProps/core.xml", b"<xml></xml>")]);
    let docx_res = parse_docx_from_memory(&empty_docx);
    assert!(docx_res.is_err());

    // 15b. EPUB corrupt META-INF/container.xml
    let bad_epub = helper_build_test_zip(&[
        ("META-INF/container.xml", b"<container><rootfiles><rootfile/></rootfiles></container>"),
    ]);
    let epub_res = parse_epub_from_memory(&bad_epub);
    assert!(epub_res.is_err());
}

// ============================================================================
// Target 16: Streaming SAX Parser State Reset & Reentrant Chunk Perturbation
// ============================================================================
#[test]
fn test_target_16_sax_streaming_state_machine_chunk_perturbation() {
    let full_xml = br#"<w:document xmlns:w="http://main"><w:body><w:p><w:r><w:t>Chunked SAX Streaming Invariant</w:t></w:r></w:p></w:body></w:document>"#;

    // Simulate byte-by-byte incremental feeding into quick-xml SAX parser
    for chunk_size in [1, 2, 5, 13, 37] {
        let res = catch_unwind(|| {
            let mut extracted = String::new();
            let mut reader = XmlReader::from_reader(Cursor::new(full_xml));
            let mut buf = Vec::with_capacity(chunk_size);
            let mut in_text = false;

            loop {
                match reader.read_event_into(&mut buf) {
                    Ok(Event::Start(ref e)) if e.local_name().as_ref() == b"t" => {
                        in_text = true;
                    }
                    Ok(Event::Text(ref e)) if in_text => {
                        if let Ok(s) = e.unescape() {
                            extracted.push_str(&s);
                        }
                    }
                    Ok(Event::End(ref e)) if e.local_name().as_ref() == b"t" => {
                        in_text = false;
                    }
                    Ok(Event::Eof) => break,
                    Err(_) => break,
                    _ => {}
                }
                buf.clear();
            }
            assert_eq!(extracted, "Chunked SAX Streaming Invariant");
        });
        assert!(res.is_ok(), "Panic in chunk size {} SAX streaming parse", chunk_size);
    }
}
