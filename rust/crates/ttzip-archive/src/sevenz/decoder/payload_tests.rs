// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use super::*;

#[test]
fn test_decode_raw_payload_chunked_brotli_passthrough() {
    let test_data = b"Brotli direct pass-through test payload without 0x184D2A50 magic framing.";
    let mut compressed = Vec::new();
    {
        let mut writer = brotli::CompressorWriter::new(&mut compressed, 4096, 6, 22);
        std::io::Write::write_all(&mut writer, test_data).unwrap();
    }

    let mut output = Vec::new();
    let total = decode_raw_payload_chunked(
        &compressed,
        METHOD_BROTLI,
        &[],
        test_data.len() as u64,
        1,
        |chunk| {
            output.extend_from_slice(chunk);
            Ok(())
        },
    )
    .expect("brotli decompression should succeed");

    assert_eq!(total, test_data.len() as u64);
    assert_eq!(output, test_data);
}

#[test]
fn test_decode_raw_payload_chunked_deflate_exceeds_memory_limit() {
    let bomb_size = (MAX_IN_MEMORY_DECOMPRESS_BYTES as u64) + 1;
    let res = decode_raw_payload_chunked(
        &[0u8; 16],
        METHOD_DEFLATE,
        &[],
        bomb_size,
        1,
        |_| Ok(()),
    );
    assert_eq!(res, Err(TTZipStatus::ErrOutOfMemory));
}

#[test]
fn test_decode_raw_payload_chunked_bzip2_exceeds_memory_limit() {
    let bomb_size = (MAX_IN_MEMORY_DECOMPRESS_BYTES as u64) + 1;
    let res = decode_raw_payload_chunked(
        &[0u8; 16],
        METHOD_BZIP2,
        &[],
        bomb_size,
        1,
        |_| Ok(()),
    );
    assert_eq!(res, Err(TTZipStatus::ErrOutOfMemory));
}

#[test]
fn test_decode_buffered_codec_rejects_above_64mib_even_with_low_ratio() {
    // 128 KiB compressed payload with 64 MiB + 1 unpack size has a compression ratio ~512 (< 1024),
    // which previously bypassed the 64 MiB ceiling up to 256 MiB.
    let raw_payload = vec![0u8; 128 * 1024];
    let over_64mib = (MAX_IN_MEMORY_DECOMPRESS_BYTES as u64) + 1;

    for method in [METHOD_DEFLATE, METHOD_BZIP2] {
        let res = decode_raw_payload_chunked(
            &raw_payload,
            method,
            &[],
            over_64mib,
            1,
            |_| Ok(()),
        );
        assert_eq!(
            res,
            Err(TTZipStatus::ErrOutOfMemory),
            "Buffered codec 0x{method:x} must reject expected_unpack_size > 64 MiB regardless of ratio"
        );
    }
}

#[test]
fn test_decode_7z_folder_streaming_single_coder_bomb_exceeds_memory_limit() {
    let dummy_payload = [0u8; 32];
    let bomb_size = (MAX_IN_MEMORY_DECOMPRESS_BYTES as u64) + 1;
    let folder = crate::sevenz::header::SevenZFolder {
        coders: vec![crate::sevenz::header::SevenZCoder {
            method_id: METHOD_COPY,
            properties: Vec::new(),
            num_in_streams: 1,
            num_out_streams: 1,
        }],
        unpack_sizes: vec![bomb_size],
        packed_offset: 0,
        packed_len: dummy_payload.len(),
        ..Default::default()
    };
    let info = SevenZHeaderInfo {
        folders: vec![folder],
        ..Default::default()
    };
    let res = decode_7z_folder_streaming(&dummy_payload, &info, 0, None, 1, |_| Ok(()));
    assert_eq!(res, Err(TTZipStatus::ErrOutOfMemory));
}

#[test]
fn test_decode_7z_packed_folder_slice_bounded_enforces_aggregate_aes_memory_ceiling() {
    // 40 MiB AES-encrypted packed slice + 32 MiB unpack budget:
    // decrypt_folder_payload_in_memory would allocate 40 MiB decrypted_storage,
    // which exceeds the remaining 32 MiB budget.
    let raw_packed = vec![0u8; 40 * 1024 * 1024];
    let folder = crate::sevenz::header::SevenZFolder {
        coders: vec![
            crate::sevenz::header::SevenZCoder {
                method_id: METHOD_AES,
                properties: vec![0x01, 0x00],
                num_in_streams: 1,
                num_out_streams: 1,
            },
            crate::sevenz::header::SevenZCoder {
                method_id: METHOD_COPY,
                properties: Vec::new(),
                num_in_streams: 1,
                num_out_streams: 1,
            },
        ],
        unpack_sizes: vec![40 * 1024 * 1024, 20 * 1024 * 1024],
        packed_offset: 0,
        packed_len: raw_packed.len(),
        ..Default::default()
    };
    let info = SevenZHeaderInfo {
        folders: vec![folder.clone()],
        is_encrypted: true,
        ..Default::default()
    };

    let res = decode_7z_packed_folder_slice_bounded(
        &raw_packed,
        &info,
        &folder,
        Some("secret"),
        1,
        Some(32 * 1024 * 1024),
        |_| Ok(()),
    );
    assert_eq!(res, Err(TTZipStatus::ErrOutOfMemory));
}

#[test]
fn test_decode_raw_payload_chunked_zstd_passthrough() {
    let test_data = b"7z METHOD_ZSTD (0x04F71101) streaming decompression verification.";
    let mut comp_buf = [0u8; 256];
    let n = crate::codecs::zstd::zstd_compress_ldm(test_data, &mut comp_buf, 3, 1).unwrap();
    let mut output = Vec::new();
    let total = decode_raw_payload_chunked(
        &comp_buf[..n],
        METHOD_ZSTD,
        &[],
        test_data.len() as u64,
        0,
        |c| {
            output.extend_from_slice(c);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(total, test_data.len() as u64);
    assert_eq!(output, test_data);
}
