// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use super::*;

const LZMA2_PREVIEW_MIB: usize = 1024 * 1024;
const LZMA2_PREVIEW_ENTRY: &str = "01-target.bin";
const LZMA2_PREVIEW_TARGET: &[u8] = b"TTZIP-654 native LZMA2 target\n";

fn read_lzma2_preview_fixture(
    path: &str,
    max_packed_mib: usize,
    max_decoded_mib: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, TTZipError> {
    super::solid_preview::extract_solid_preview_from_path(
        Path::new(path),
        LZMA2_PREVIEW_ENTRY,
        crate::sevenz::decoder::stream::SolidLzma2Preview {
            max_packed_bytes: max_packed_mib * LZMA2_PREVIEW_MIB,
            max_decoded_bytes: (max_decoded_mib * LZMA2_PREVIEW_MIB) as u64,
            cancellation: cancelled,
        },
    )
}

#[test]
#[ignore = "requires the retained synthetic native and high-ratio 7z fixtures"]
fn solid_lzma2_by_path_experiment() {
    let regular = std::env::var("TTZIP_NATIVE_PROBE_ARCHIVE").unwrap();
    let ratio = std::env::var("TTZIP_RATIO_PROBE_ARCHIVE").unwrap();
    let standard = extract_single_entry_by_path(regular.clone(), LZMA2_PREVIEW_ENTRY.into(), None);
    assert!(matches!(
        standard,
        Err(TTZipError::DecompressionBombDetected { .. })
    ));
    assert_eq!(
        read_lzma2_preview_fixture(&regular, 40, 66, &|| false).unwrap(),
        LZMA2_PREVIEW_TARGET
    );
    for result in [
        read_lzma2_preview_fixture(&regular, 8, 66, &|| false),
        read_lzma2_preview_fixture(&regular, 40, 64, &|| false),
        read_lzma2_preview_fixture(&ratio, 40, 66, &|| false),
    ] {
        assert!(matches!(
            result,
            Err(TTZipError::DecompressionBombDetected { .. })
        ));
    }

    let token = CancellationToken::new();
    let polls = std::cell::Cell::new(0);
    let cancelled = || {
        polls.set(polls.get() + 1);
        if polls.get() == 9 {
            token.cancel();
        }
        token.is_cancelled()
    };
    assert!(matches!(
        read_lzma2_preview_fixture(&regular, 40, 66, &cancelled),
        Err(TTZipError::EngineError { code }) if code == TTZipStatus::Cancelled as i32
    ));
    assert_eq!(polls.get(), 9);
}

#[test]
#[ignore = "requires retained synthetic archive variants with deliberate corruption"]
fn solid_lzma2_by_path_rejects_invalid_archive_bytes() {
    let directory = std::env::var("TTZIP_SOLID_NEGATIVE_DIR").unwrap();
    for name in [
        "bad-crc.7z",
        "invalid-lzma2-control.7z",
        "truncated-archive.7z",
    ] {
        let path = Path::new(&directory).join(name);
        let result = read_lzma2_preview_fixture(path.to_str().unwrap(), 40, 66, &|| false);
        assert!(
            matches!(result, Err(TTZipError::CorruptHeader { .. })),
            "{name}: {result:?}"
        );
    }
    let path = Path::new(&directory).join("large-dictionary.7z");
    let result = read_lzma2_preview_fixture(path.to_str().unwrap(), 40, 66, &|| false);
    assert!(
        matches!(result, Err(TTZipError::OutOfMemory { .. })),
        "{result:?}"
    );
}

#[test]
#[ignore = "requires retained synthetic metadata fixtures"]
fn solid_lzma2_by_path_rejects_metadata_before_decode() {
    let directory = std::env::var("TTZIP_SOLID_NEGATIVE_DIR").unwrap();
    let missing = Path::new(&directory).join("never-open.7z");
    assert!(matches!(
        read_lzma2_preview_fixture(missing.to_str().unwrap(), 40, 66, &|| true),
        Err(TTZipError::EngineError { code }) if code == TTZipStatus::Cancelled as i32
    ));

    let split = Path::new(&directory).join("never-open.7z.001");
    assert!(matches!(
        read_lzma2_preview_fixture(split.to_str().unwrap(), 40, 66, &|| false),
        Err(TTZipError::EngineError { code }) if code == TTZipStatus::ErrUnsupportedFeature as i32
    ));

    for name in [
        "oversized-next-header.7z",
        "too-many-files.7z",
        "too-many-substreams.7z",
    ] {
        let path = Path::new(&directory).join(name);
        let result = read_lzma2_preview_fixture(path.to_str().unwrap(), 40, 66, &|| false);
        assert!(
            matches!(result, Err(TTZipError::DecompressionBombDetected { .. })),
            "{name}: {result:?}"
        );
    }
    let path = Path::new(&directory).join("encoded-next-header.7z");
    let result = read_lzma2_preview_fixture(path.to_str().unwrap(), 40, 66, &|| false);
    assert!(
        matches!(
            result, Err(TTZipError::EngineError { code }) if code == TTZipStatus::ErrUnsupportedFeature as i32
        ),
        "{result:?}"
    );
}

#[test]
#[ignore = "requires the independent review's sparse and complete CRC fixtures"]
fn solid_lzma2_by_path_rejects_sparse_substream_crc() {
    let directory = std::env::var("TTZIP_SOLID_NEGATIVE_DIR").unwrap();
    let sparse = Path::new(&directory).join("review-sparse-substream-crc.7z");
    let result = read_lzma2_preview_fixture(sparse.to_str().unwrap(), 40, 66, &|| false);
    assert!(
        matches!(
            result, Err(TTZipError::EngineError { code })
                if code == TTZipStatus::ErrUnsupportedFeature as i32
        ),
        "{result:?}"
    );

    let complete = std::env::var("TTZIP_NATIVE_PROBE_ARCHIVE").unwrap();
    let bytes = read_lzma2_preview_fixture(&complete, 40, 66, &|| false).unwrap();
    assert_eq!(bytes, LZMA2_PREVIEW_TARGET);
}
