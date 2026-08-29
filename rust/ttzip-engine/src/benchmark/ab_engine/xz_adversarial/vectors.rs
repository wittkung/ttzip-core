// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Canonical 98 XZ Adversarial Vector Generator & Threat Classification.

use crate::benchmark::ab_engine::xz_adversarial::validator::XZ_MAGIC_FOOTER;
use crate::crypto::crc32::crc32_fast;

/// Category classification of XZ adversarial test vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XzAdversarialCategory {
    /// VLI integer overflow (>2^63-1 or >9 bytes) or non-minimal encoding.
    VliOverflow,
    /// Header magic corruption, reserved flags, or header CRC32 mismatch.
    HeaderCorruption,
    /// Footer magic corruption, backward size mismatch, or flags parity mismatch.
    FooterCorruption,
    /// Block header ending mid-filter, unknown filter ID, or zero/huge compressed size.
    BlockHeaderCorruption,
    /// CRC32, CRC64, or SHA-256 checksum fraud.
    CrcFraud,
    /// LZMA2 uninitialized dictionary, illegal lc/lp/pb, or reserved control bytes.
    Lzma2StateBacktrack,
    /// Index uncompressed size overflow, record count bomb, or padding corruption.
    IndexBombAndOverflow,
    /// Stream padding not multiple of 4 bytes or containing non-null bytes.
    StreamPaddingCorruption,
    /// Filter chain structure violation (e.g. Delta as last filter).
    FilterFlagViolation,
    /// Unsupported feature flag or unknown check ID.
    UnsupportedFeature,
}

/// A standalone adversarial test vector with metadata and expected outcome.
#[derive(Debug, Clone)]
pub struct XzAdversarialVector {
    /// Unique identifier / filename of the vector.
    pub name: String,
    /// Threat category.
    pub category: XzAdversarialCategory,
    /// Descriptive technical summary of the attack vector.
    pub description: String,
    /// Raw adversarial payload.
    pub payload: Vec<u8>,
    /// Expected error category substring.
    pub expected_error: String,
}

/// Helper to construct standard valid 12-byte XZ header.
fn make_header(flags: [u8; 2]) -> Vec<u8> {
    let mut h = vec![0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00];
    h.extend_from_slice(&flags);
    let crc = crc32_fast(0, &flags);
    h.extend_from_slice(&crc.to_le_bytes());
    h
}

/// Helper to construct standard valid 12-byte XZ footer.
fn make_footer(flags: [u8; 2], backward_size: u32) -> Vec<u8> {
    let mut f = Vec::with_capacity(12);
    let mut payload = Vec::new();
    payload.extend_from_slice(&backward_size.to_le_bytes());
    payload.extend_from_slice(&flags);
    let crc = crc32_fast(0, &payload);
    f.extend_from_slice(&crc.to_le_bytes());
    f.extend_from_slice(&payload);
    f.extend_from_slice(&XZ_MAGIC_FOOTER);
    f
}

/// Generates the canonical 98 adversarial and edge vectors.
pub fn generate_98_adversarial_suite() -> Vec<XzAdversarialVector> {
    let mut suite = Vec::with_capacity(98);

    // 1..10: VLI integer encoding & overflow vectors
    suite.push(XzAdversarialVector {
        name: "bad-1-vli-1.xz".into(),
        category: XzAdversarialCategory::VliOverflow,
        description: "2-byte VLI encoding for value <= 127 (non-minimal VLI violation)".into(),
        payload: {
            let mut v = make_header([0, 1]);
            v.extend_from_slice(&[0x02, 0x80, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00]);
            v
        },
        expected_error: "VLI".into(),
    });

    suite.push(XzAdversarialVector {
        name: "bad-1-vli-2.xz".into(),
        category: XzAdversarialCategory::VliOverflow,
        description: "10-byte VLI integer overflow > 63 bits".into(),
        payload: {
            let mut v = make_header([0, 1]);
            v.extend_from_slice(&[0x02, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F]);
            v
        },
        expected_error: "overflow".into(),
    });

    for i in 3..=10 {
        suite.push(XzAdversarialVector {
            name: format!("bad-1-vli-synthetic-{i}.xz"),
            category: XzAdversarialCategory::VliOverflow,
            description: format!("Synthetic VLI boundary corruption sequence variant #{i}"),
            payload: {
                let mut v = make_header([0, 1]);
                let mut block = vec![0x03, 0x00];
                block.resize(i + 4, 0x80 | (i as u8));
                block.push(0x00);
                v.extend_from_slice(&block);
                v
            },
            expected_error: "VLI".into(),
        });
    }

    // 11..25: Header magic, flags, and CRC32 corruption vectors
    suite.push(XzAdversarialVector {
        name: "bad-0-header_magic.xz".into(),
        category: XzAdversarialCategory::HeaderCorruption,
        description: "Header magic byte 0 changed from 0xFD to 0xFE".into(),
        payload: vec![0xFE, 0x37, 0x7A, 0x58, 0x5A, 0x00, 0, 1, 0, 0, 0, 0],
        expected_error: "magic".into(),
    });

    suite.push(XzAdversarialVector {
        name: "bad-1-stream_flags-2.xz".into(),
        category: XzAdversarialCategory::HeaderCorruption,
        description: "Fraudulent header CRC32 in stream flags".into(),
        payload: vec![0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00, 0, 1, 0xDE, 0xAD, 0xBE, 0xEF],
        expected_error: "Checksum".into(),
    });

    suite.push(XzAdversarialVector {
        name: "bad-0-empty-truncated.xz".into(),
        category: XzAdversarialCategory::HeaderCorruption,
        description: "Stream truncated before completing 12-byte stream header".into(),
        payload: vec![0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00, 0],
        expected_error: "truncated".into(),
    });

    for i in 4..=15 {
        suite.push(XzAdversarialVector {
            name: format!("bad-0-header-corrupt-var-{i}.xz"),
            category: XzAdversarialCategory::HeaderCorruption,
            description: format!("Header corruption permutation #{i} with invalid reserved bits"),
            payload: {
                let mut v = make_header([(i as u8) | 0x80, 1]);
                v[6] = (i as u8) | 0x01;
                v
            },
            expected_error: "flags".into(),
        });
    }

    // 26..38: Footer corruption & flags parity vectors
    suite.push(XzAdversarialVector {
        name: "bad-0-footer_magic.xz".into(),
        category: XzAdversarialCategory::FooterCorruption,
        description: "Footer magic byte changed from YZ to XZ".into(),
        payload: {
            let mut v = make_header([0, 1]);
            let mut footer = make_footer([0, 1], 0);
            footer[10] = b'X';
            v.extend_from_slice(&footer);
            v
        },
        expected_error: "Footer".into(),
    });

    suite.push(XzAdversarialVector {
        name: "bad-1-stream_flags-1.xz".into(),
        category: XzAdversarialCategory::FooterCorruption,
        description: "Stream flags parity mismatch between header and footer".into(),
        payload: {
            let mut v = make_header([0, 1]);
            v.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00]);
            v.extend_from_slice(&make_footer([0, 4], 0));
            v
        },
        expected_error: "mismatch".into(),
    });

    suite.push(XzAdversarialVector {
        name: "bad-0-backward_size.xz".into(),
        category: XzAdversarialCategory::FooterCorruption,
        description: "Backward size field in footer does not match index size".into(),
        payload: {
            let mut v = make_header([0, 1]);
            v.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00]);
            v.extend_from_slice(&make_footer([0, 1], 9999));
            v
        },
        expected_error: "Backward size".into(),
    });

    for i in 4..=13 {
        suite.push(XzAdversarialVector {
            name: format!("bad-0-footer-synthetic-{i}.xz"),
            category: XzAdversarialCategory::FooterCorruption,
            description: format!("Footer CRC and Backward size perturbation #{i}"),
            payload: {
                let mut v = make_header([0, 1]);
                let mut f = make_footer([0, 1], (i * 16) as u32);
                f[0] ^= i as u8;
                v.extend_from_slice(&f);
                v
            },
            expected_error: "Footer".into(),
        });
    }

    // 39..55: Block Header, Filter Flags & Filter Chain Corruption
    suite.push(XzAdversarialVector {
        name: "bad-1-block_header-1.xz".into(),
        category: XzAdversarialCategory::BlockHeaderCorruption,
        description: "Block header truncated in the middle of filter flags".into(),
        payload: {
            let mut v = make_header([0, 1]);
            v.extend_from_slice(&[0x02, 0x00, 0x21]);
            v
        },
        expected_error: "truncated".into(),
    });

    suite.push(XzAdversarialVector {
        name: "bad-1-block_header-2.xz".into(),
        category: XzAdversarialCategory::BlockHeaderCorruption,
        description: "Block header specifies compressed size but has no filters".into(),
        payload: {
            let mut v = make_header([0, 1]);
            let bh = [0x01, 0x40, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00];
            v.extend_from_slice(&bh);
            v
        },
        expected_error: "header".into(),
    });

    suite.push(XzAdversarialVector {
        name: "unsupported-filter_flags-2.xz".into(),
        category: XzAdversarialCategory::FilterFlagViolation,
        description: "Delta filter (0x03) illegal as last filter in chain".into(),
        payload: {
            let mut v = make_header([0, 1]);
            let mut bh = vec![0x01, 0x00, 0x03, 0x01, 0x00];
            while (bh.len() + 4) % 4 != 0 {
                bh.push(0x00);
            }
            let crc = crc32_fast(0, &bh);
            bh.extend_from_slice(&crc.to_le_bytes());
            bh[0] = ((bh.len() / 4) - 1) as u8;
            let crc_corrected = crc32_fast(0, &bh[..bh.len() - 4]);
            let len = bh.len();
            bh[len - 4..].copy_from_slice(&crc_corrected.to_le_bytes());
            v.extend_from_slice(&bh);
            v
        },
        expected_error: "Delta".into(),
    });

    suite.push(XzAdversarialVector {
        name: "unsupported-filter_flags-3.xz".into(),
        category: XzAdversarialCategory::FilterFlagViolation,
        description: "Multiple LZMA2 filters in filter chain".into(),
        payload: {
            let mut v = make_header([0, 1]);
            let mut bh = vec![0x03, 0x01, 0x21, 0x01, 0x00, 0x21, 0x01, 0x00];
            while (bh.len() + 4) % 4 != 0 {
                bh.push(0x00);
            }
            let crc = crc32_fast(0, &bh);
            bh.extend_from_slice(&crc.to_le_bytes());
            v.extend_from_slice(&bh);
            v
        },
        expected_error: "LZMA2".into(),
    });

    for i in 5..=17 {
        suite.push(XzAdversarialVector {
            name: format!("bad-1-block_header-synthetic-{i}.xz"),
            category: XzAdversarialCategory::BlockHeaderCorruption,
            description: format!("Block Header compressed size / CRC mutation #{i}"),
            payload: {
                let mut v = make_header([0, 1]);
                let mut bh = vec![0x01, 0x00, 0x21, 0x01, i as u8];
                while (bh.len() + 4) % 4 != 0 {
                    bh.push(0x00);
                }
                bh.extend_from_slice(&[0xBA, 0xAD, 0xF0, 0x0D]);
                v.extend_from_slice(&bh);
                v
            },
            expected_error: "Header".into(),
        });
    }

    // 56..70: LZMA2 State Machine & Dictionary Backtrack Vectors
    suite.push(XzAdversarialVector {
        name: "bad-1-lzma2-1.xz".into(),
        category: XzAdversarialCategory::Lzma2StateBacktrack,
        description: "First LZMA2 chunk (uncompressed) does not reset dictionary (0x02 control byte)".into(),
        payload: {
            let mut v = make_header([0, 1]);
            let mut bh = vec![0x01, 0x40, 0x04, 0x21, 0x01, 0x00];
            while (bh.len() + 4) % 4 != 0 {
                bh.push(0x00);
            }
            let crc = crc32_fast(0, &bh);
            bh.extend_from_slice(&crc.to_le_bytes());
            bh[0] = ((bh.len() / 4) - 1) as u8;
            let crc_cor = crc32_fast(0, &bh[..bh.len() - 4]);
            let len = bh.len();
            bh[len - 4..].copy_from_slice(&crc_cor.to_le_bytes());
            v.extend_from_slice(&bh);
            v.extend_from_slice(&[0x02, 0x00, 0x01, b'A']);
            v
        },
        expected_error: "dictionary".into(),
    });

    suite.push(XzAdversarialVector {
        name: "bad-1-lzma2-6.xz".into(),
        category: XzAdversarialCategory::Lzma2StateBacktrack,
        description: "Reserved LZMA2 control byte value 0x03".into(),
        payload: {
            let mut v = make_header([0, 1]);
            let mut bh = vec![0x01, 0x40, 0x04, 0x21, 0x01, 0x00];
            while (bh.len() + 4) % 4 != 0 {
                bh.push(0x00);
            }
            let crc = crc32_fast(0, &bh);
            bh.extend_from_slice(&crc.to_le_bytes());
            bh[0] = ((bh.len() / 4) - 1) as u8;
            let crc_cor = crc32_fast(0, &bh[..bh.len() - 4]);
            let len = bh.len();
            bh[len - 4..].copy_from_slice(&crc_cor.to_le_bytes());
            v.extend_from_slice(&bh);
            v.extend_from_slice(&[0x03, 0x00, 0x00, 0x00]);
            v
        },
        expected_error: "control byte".into(),
    });

    for i in 3..=15 {
        suite.push(XzAdversarialVector {
            name: format!("bad-1-lzma2-synthetic-{i}.xz"),
            category: XzAdversarialCategory::Lzma2StateBacktrack,
            description: format!("LZMA2 control byte reservation and state violation #{i}"),
            payload: {
                let mut v = make_header([0, 1]);
                let mut bh = vec![0x01, 0x40, 0x04, 0x21, 0x01, 0x00];
                while (bh.len() + 4) % 4 != 0 {
                    bh.push(0x00);
                }
                let crc = crc32_fast(0, &bh);
                bh.extend_from_slice(&crc.to_le_bytes());
                bh[0] = ((bh.len() / 4) - 1) as u8;
                let crc_cor = crc32_fast(0, &bh[..bh.len() - 4]);
                let len = bh.len();
                bh[len - 4..].copy_from_slice(&crc_cor.to_le_bytes());
                v.extend_from_slice(&bh);
                v.push((0x04 + (i as u8)) & 0x7F);
                v.extend_from_slice(&[0x00, 0x00, 0x00]);
                v
            },
            expected_error: "control byte".into(),
        });
    }

    // 71..85: Index Bomb, Uncompressed Overflow & Index Corruption
    suite.push(XzAdversarialVector {
        name: "bad-1-index-huge-uncomp.xz".into(),
        category: XzAdversarialCategory::IndexBombAndOverflow,
        description: "Index record uncompressed size exceeds UINT64_MAX / 3".into(),
        payload: {
            let mut v = make_header([0, 1]);
            let mut idx = vec![0x00, 0x01, 0x10];
            idx.extend_from_slice(&[0xD6, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0x55]);
            while idx.len() % 4 != 0 {
                idx.push(0x00);
            }
            let crc = crc32_fast(0, &idx);
            idx.extend_from_slice(&crc.to_le_bytes());
            v.extend_from_slice(&idx);
            v.extend_from_slice(&make_footer([0, 1], (idx.len() / 4 - 1) as u32));
            v
        },
        expected_error: "Index".into(),
    });

    suite.push(XzAdversarialVector {
        name: "bad-0-nonempty_index.xz".into(),
        category: XzAdversarialCategory::IndexBombAndOverflow,
        description: "Index claims 1 record when 0 blocks were decoded in stream".into(),
        payload: {
            let mut v = make_header([0, 1]);
            let mut idx = vec![0x00, 0x01, 0x10, 0x10];
            while idx.len() % 4 != 0 {
                idx.push(0x00);
            }
            let crc = crc32_fast(0, &idx);
            idx.extend_from_slice(&crc.to_le_bytes());
            v.extend_from_slice(&idx);
            v.extend_from_slice(&make_footer([0, 1], (idx.len() / 4 - 1) as u32));
            v
        },
        expected_error: "zero blocks".into(),
    });

    for i in 3..=15 {
        suite.push(XzAdversarialVector {
            name: format!("bad-2-index-synthetic-{i}.xz"),
            category: XzAdversarialCategory::IndexBombAndOverflow,
            description: format!("Index CRC mismatch and unpadded size zero mutation #{i}"),
            payload: {
                let mut v = make_header([0, 1]);
                let mut idx = vec![0x00, 0x01, 0x00, 0x10];
                while idx.len() % 4 != 0 {
                    idx.push(0x00);
                }
                idx.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
                v.extend_from_slice(&idx);
                v
            },
            expected_error: "Index".into(),
        });
    }

    // 86..98: Stream Padding & Checksum Fraud Vectors
    suite.push(XzAdversarialVector {
        name: "bad-0pad-empty.xz".into(),
        category: XzAdversarialCategory::StreamPaddingCorruption,
        description: "Stream padding has 5 bytes (must be a multiple of 4)".into(),
        payload: {
            let mut v = make_header([0, 1]);
            let idx = vec![0x00, 0x00, 0x00, 0x00, 0x00];
            v.extend_from_slice(&idx);
            v.extend_from_slice(&make_footer([0, 1], 0));
            v.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00]);
            v
        },
        expected_error: "padding".into(),
    });

    suite.push(XzAdversarialVector {
        name: "bad-1-check-crc32.xz".into(),
        category: XzAdversarialCategory::CrcFraud,
        description: "Check CRC32 value corrupted in data payload".into(),
        payload: {
            let mut v = make_header([0, 1]);
            let mut bh = vec![0x01, 0x40, 0x04, 0x21, 0x01, 0x00];
            while (bh.len() + 4) % 4 != 0 {
                bh.push(0x00);
            }
            let crc = crc32_fast(0, &bh);
            bh.extend_from_slice(&crc.to_le_bytes());
            bh[0] = ((bh.len() / 4) - 1) as u8;
            let crc_cor = crc32_fast(0, &bh[..bh.len() - 4]);
            let len = bh.len();
            bh[len - 4..].copy_from_slice(&crc_cor.to_le_bytes());
            v.extend_from_slice(&bh);
            v.extend_from_slice(&[0x01, 0x00, 0x01, b'Z']);
            v.extend_from_slice(&[0xBA, 0xAD, 0xF0, 0x0D]);
            v
        },
        expected_error: "Checksum".into(),
    });

    for i in 3..=13 {
        suite.push(XzAdversarialVector {
            name: format!("bad-0pad-synthetic-{i}.xz"),
            category: XzAdversarialCategory::StreamPaddingCorruption,
            description: format!("Stream padding non-zero dirty byte attack #{i}"),
            payload: {
                let mut v = make_header([0, 1]);
                v.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00]);
                v.extend_from_slice(&make_footer([0, 1], 0));
                v.extend_from_slice(&[0x00, 0x00, i as u8, 0x00]);
                v
            },
            expected_error: "padding".into(),
        });
    }

    assert_eq!(suite.len(), 98, "Suite must contain exactly 98 adversarial vectors");
    suite
}
