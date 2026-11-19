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

#[test]
#[ignore = "requires retained synthetic #654 admission and compatibility fixtures"]
fn automatic_preview_preserves_legacy_and_rejects_strict_failures() {
    let outputs = std::env::var("TTZIP_654_OUTPUTS_DIR").unwrap();
    let read = |relative: &str| {
        let path = Path::new(&outputs).join(relative);
        extract_single_entry_for_preview(
            path.to_str().unwrap().to_owned(),
            LZMA2_PREVIEW_ENTRY.to_owned(),
            None,
            &CancellationToken::new(),
        )
    };
    assert_eq!(
        read("tzip-654-lzma2-probe/native-probe.7z").unwrap(),
        LZMA2_PREVIEW_TARGET
    );
    for name in ["ordinary-early.7z", "ordinary-stored.7z", "ordinary.zip"] {
        let relative = format!("tzip-654-auto-preview-acceptance/controls/{name}");
        assert_eq!(read(&relative).unwrap(), LZMA2_PREVIEW_TARGET, "{name}");
    }
    for name in ["short-header-1025-files.7z", "big-header-701-files.7z"] {
        let relative = format!("tzip-654-auto-preview/independent-compatibility/{name}");
        assert_eq!(read(&relative).unwrap(), b"ordinary preview", "{name}");
    }
    assert!(matches!(
        read("tzip-654-lzma2-probe/ratio-probe.7z"),
        Err(ArchiveEntryPreviewError::Terminal(TTZipError::DecompressionBombDetected { .. }))
    ));
    for name in ["bad-crc.7z", "invalid-lzma2-control.7z"] {
        let relative = format!("tzip-654-lzma2-integration/{name}");
        assert!(
            matches!(read(&relative), Err(ArchiveEntryPreviewError::Terminal(TTZipError::CorruptHeader { .. }))),
            "{name}",
        );
    }
    assert!(matches!(
        read("tzip-654-lzma2-integration/large-dictionary.7z"),
        Err(ArchiveEntryPreviewError::Terminal(TTZipError::OutOfMemory { .. }))
    ));
    assert!(matches!(
        read("tzip-654-lzma2-integration/review-sparse-substream-crc.7z"),
        Err(ArchiveEntryPreviewError::Terminal(TTZipError::SecurityViolation { .. }))
    ));
    assert!(matches!(
        read("tzip-654-auto-preview/independent-review/overflow-coder-props.7z"),
        Err(ArchiveEntryPreviewError::Legacy(TTZipError::CorruptHeader { .. }))
    ));
}

#[test]
#[ignore = "requires the retained synthetic #654 admission fixture"]
fn automatic_preview_decodes_the_same_open_input() {
    let outputs = std::env::var("TTZIP_654_OUTPUTS_DIR").unwrap();
    let original = Path::new(&outputs).join("tzip-654-lzma2-probe/native-probe.7z");
    let temporary = tempfile::tempdir().unwrap();
    let archive = temporary.path().join("unlink.7z");
    std::fs::copy(&original, &archive).unwrap();
    let polls = std::cell::Cell::new(0);
    let unlink_after_open = || {
        polls.set(polls.get() + 1);
        if polls.get() == 2 {
            std::fs::remove_file(&archive).unwrap();
        }
        false
    };
    let archive_path = archive.to_str().unwrap().to_owned();
    let read = super::entry_preview::extract_single_entry(
        archive_path.clone(), LZMA2_PREVIEW_ENTRY.into(), None, Some(&unlink_after_open),
    );
    assert_eq!(read.unwrap(), LZMA2_PREVIEW_TARGET);
    assert!(!archive.exists());

    let token = CancellationToken::new();
    token.cancel();
    assert!(matches!(
        extract_single_entry_for_preview(archive_path, LZMA2_PREVIEW_ENTRY.into(), None, &token),
        Err(ArchiveEntryPreviewError::Terminal(TTZipError::Cancelled))
    ));
}

fn wrap_7z_archive(payload: &[u8], next_header: &[u8]) -> Vec<u8> {
    use crate::crypto::crc32::crc32_fast;
    use crate::sevenz::format::SevenZSignatureHeader;

    let signature = SevenZSignatureHeader {
        major_version: 0,
        minor_version: 4,
        start_header_crc: 0,
        next_header_offset: payload.len() as u64,
        next_header_size: next_header.len() as u64,
        next_header_crc: crc32_fast(0, next_header),
    }
    .serialize();
    let mut out = Vec::with_capacity(signature.len() + payload.len() + next_header.len());
    out.extend_from_slice(&signature);
    out.extend_from_slice(payload);
    out.extend_from_slice(next_header);
    out
}

fn build_plain_lzma2_fixture_parts() -> (Vec<u8>, Vec<u8>) {
    use crate::crypto::crc32::crc32_fast;
    use crate::sevenz::format::METHOD_LZMA2;
    use crate::sevenz::writer::build_7z_metadata_header;
    use crate::zip::writer::ZipInputItem;

    let tail = b"02-tail\n";
    let mut payload = Vec::from(LZMA2_PREVIEW_TARGET);
    payload.extend_from_slice(tail);
    let items = [
        (LZMA2_PREVIEW_ENTRY, LZMA2_PREVIEW_TARGET),
        ("02-tail.bin", tail.as_slice()),
    ]
    .map(|(rel_path, data)| ZipInputItem {
        rel_path: rel_path.to_string(),
        data: data.to_vec(),
        mtime_epoch_secs: 0,
        mode: 0o100644,
        is_directory: false,
    });
    let sizes = [LZMA2_PREVIEW_TARGET.len() as u64, tail.len() as u64];
    let crcs = [crc32_fast(0, LZMA2_PREVIEW_TARGET), crc32_fast(0, tail)];
    let header = build_7z_metadata_header(
        &items,
        &sizes,
        &crcs,
        payload.len() as u64,
        payload.len() as u64,
        METHOD_LZMA2,
        &[0x00],
    );
    (payload, header)
}

fn assert_solid_preview_rejects_corrupt(temp: &Path, name: &str, archive_bytes: &[u8]) {
    let path = temp.join(name);
    std::fs::write(&path, archive_bytes).expect("write corrupt 7z fixture");
    assert_eq!(
        crate::sevenz::decoder::SevenZArchive::open_slice(archive_bytes).err(),
        Some(TTZipStatus::ErrCorruptHeader),
        "{name}: standard open_slice must reject corrupt header"
    );
    let direct = read_lzma2_preview_fixture(path.to_str().unwrap(), 40, 66, &|| false);
    assert!(
        matches!(direct, Err(TTZipError::CorruptHeader { .. })),
        "{name}: solid preview from path must return CorruptHeader, got {direct:?}"
    );
    let selected = super::solid_preview::extract_selected_solid_preview(
        archive_bytes,
        LZMA2_PREVIEW_ENTRY,
        0,
        &|| false,
    );
    assert!(
        matches!(selected, Err(TTZipError::CorruptHeader { .. })),
        "{name}: selected solid preview must return CorruptHeader, got {selected:?}"
    );
    let auto = extract_single_entry_for_preview(
        path.to_str().unwrap().to_owned(),
        LZMA2_PREVIEW_ENTRY.to_owned(),
        None,
        &CancellationToken::new(),
    );
    assert!(
        matches!(auto, Err(ArchiveEntryPreviewError::Legacy(TTZipError::CorruptHeader { .. }))),
        "{name}: automatic preview must return CorruptHeader, got {auto:?}"
    );
}

#[test]
fn solid_preview_rejects_corrupt_coder_descriptors_without_panicking() {
    let temp = tempfile::tempdir().expect("tempdir");
    let (payload, lzma2_header) = build_plain_lzma2_fixture_parts();
    let coder_pos = lzma2_header
        .windows(4)
        .position(|w| w == [0x21, 0x21, 0x01, 0x00])
        .expect("locate LZMA2 coder descriptor");

    let mut overflow_header = lzma2_header[..coder_pos + 2].to_vec();
    overflow_header.extend_from_slice(&[0xFF; 9]);
    overflow_header.extend_from_slice(&lzma2_header[coder_pos + 3..]);
    assert_solid_preview_rejects_corrupt(
        temp.path(),
        "overflow-coder-props.7z",
        &wrap_7z_archive(&payload, &overflow_header),
    );

    let mut truncated_header = lzma2_header[..coder_pos + 2].to_vec();
    truncated_header.push(0x7F);
    truncated_header.extend_from_slice(&lzma2_header[coder_pos + 3..]);
    assert_solid_preview_rejects_corrupt(
        temp.path(),
        "truncated-coder-props.7z",
        &wrap_7z_archive(&payload, &truncated_header),
    );

    let mut reserved_flags_header = lzma2_header;
    reserved_flags_header[coder_pos] = 0x61;
    assert_solid_preview_rejects_corrupt(
        temp.path(),
        "reserved-coder-flags.7z",
        &wrap_7z_archive(&payload, &reserved_flags_header),
    );
}

#[test]
fn solid_preview_rejects_duplicate_unclosed_and_trailing_header_sections() {
    let temp = tempfile::tempdir().expect("tempdir");
    let (payload, lzma2_header) = build_plain_lzma2_fixture_parts();

    let files_pos = lzma2_header
        .windows(3)
        .position(|w| w == [0x00, 0x05, 0x02])
        .expect("locate FilesInfo section")
        + 1;
    let mut dup_files_header = lzma2_header[..lzma2_header.len() - 1].to_vec();
    dup_files_header.extend_from_slice(&lzma2_header[files_pos..]);
    let dup_archive = wrap_7z_archive(&payload, &dup_files_header);
    let dup_path = temp.path().join("dup-files-info.7z");
    std::fs::write(&dup_path, &dup_archive).expect("write dup archive");

    let dup_direct = read_lzma2_preview_fixture(dup_path.to_str().unwrap(), 40, 66, &|| false);
    assert!(matches!(
        dup_direct,
        Err(TTZipError::EngineError { code }) if code == TTZipStatus::ErrUnsupportedFeature as i32
    ));
    let dup_selected = super::solid_preview::extract_selected_solid_preview(
        &dup_archive,
        LZMA2_PREVIEW_ENTRY,
        0,
        &|| false,
    );
    assert!(matches!(dup_selected, Err(TTZipError::SecurityViolation { .. })));

    let mut unclosed_header = lzma2_header.clone();
    unclosed_header.pop();
    let unclosed_archive = wrap_7z_archive(&payload, &unclosed_header);
    let unclosed_selected = super::solid_preview::extract_selected_solid_preview(
        &unclosed_archive,
        LZMA2_PREVIEW_ENTRY,
        0,
        &|| false,
    );
    assert!(matches!(unclosed_selected, Err(TTZipError::CorruptHeader { .. })));

    let mut trailing_header = lzma2_header;
    trailing_header.push(0x00);
    let trailing_archive = wrap_7z_archive(&payload, &trailing_header);
    let trailing_selected = super::solid_preview::extract_selected_solid_preview(
        &trailing_archive,
        LZMA2_PREVIEW_ENTRY,
        0,
        &|| false,
    );
    assert!(matches!(trailing_selected, Err(TTZipError::CorruptHeader { .. })));
}

#[test]
fn automatic_preview_honors_cancellation_across_plain_encoded_and_zip_routes() {
    use crate::crypto::crc32::crc32_fast;
    use crate::sevenz::format::{METHOD_COPY, build_7z_encoded_header_boot};
    use crate::sevenz::writer::build_7z_metadata_header;
    use crate::types::TTZipCreateOptions;
    use crate::zip::writer::{ZipInputItem, create_zip_archive};

    let temp = tempfile::tempdir().expect("tempdir");
    let copy_item = [ZipInputItem {
        rel_path: LZMA2_PREVIEW_ENTRY.to_string(),
        data: LZMA2_PREVIEW_TARGET.to_vec(),
        mtime_epoch_secs: 0,
        mode: 0o100644,
        is_directory: false,
    }];
    let target_crc = crc32_fast(0, LZMA2_PREVIEW_TARGET);
    let copy_header = build_7z_metadata_header(
        &copy_item,
        &[LZMA2_PREVIEW_TARGET.len() as u64],
        &[target_crc],
        LZMA2_PREVIEW_TARGET.len() as u64,
        LZMA2_PREVIEW_TARGET.len() as u64,
        METHOD_COPY,
        &[],
    );
    let plain_path = temp.path().join("plain-copy.7z");
    std::fs::write(&plain_path, wrap_7z_archive(LZMA2_PREVIEW_TARGET, &copy_header))
        .expect("write plain copy 7z");

    let mut encoded_payload = Vec::from(LZMA2_PREVIEW_TARGET);
    let boot = build_7z_encoded_header_boot(
        encoded_payload.len() as u64,
        copy_header.len() as u64,
        copy_header.len() as u64,
        crc32_fast(0, &copy_header),
        METHOD_COPY,
        &[],
        None,
    );
    encoded_payload.extend_from_slice(&copy_header);
    let encoded_path = temp.path().join("encoded-copy.7z");
    std::fs::write(&encoded_path, wrap_7z_archive(&encoded_payload, &boot))
        .expect("write encoded copy 7z");

    let entry_file = temp.path().join(LZMA2_PREVIEW_ENTRY);
    std::fs::write(&entry_file, LZMA2_PREVIEW_TARGET).expect("write zip input");
    let zip_path = temp.path().join("control.zip");
    create_zip_archive(&zip_path, &[entry_file], &TTZipCreateOptions::default())
        .expect("create zip control");

    for path in [&plain_path, &encoded_path, &zip_path] {
        let path_str = path.to_str().unwrap().to_owned();
        let bytes = extract_single_entry_for_preview(
            path_str.clone(),
            LZMA2_PREVIEW_ENTRY.to_owned(),
            None,
            &CancellationToken::new(),
        )
        .unwrap_or_else(|err| panic!("{path:?} uncancelled preview must succeed: {err:?}"));
        assert_eq!(bytes, LZMA2_PREVIEW_TARGET);

        let token = CancellationToken::new();
        token.cancel();
        let cancelled = extract_single_entry_for_preview(
            path_str,
            LZMA2_PREVIEW_ENTRY.to_owned(),
            None,
            &token,
        );
        assert!(
            matches!(cancelled, Err(ArchiveEntryPreviewError::Terminal(TTZipError::Cancelled))),
            "{path:?} cancelled preview must return terminal Cancelled: {cancelled:?}"
        );
    }
}
