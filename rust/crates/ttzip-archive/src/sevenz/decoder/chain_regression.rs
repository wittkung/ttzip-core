// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Independent regression coverage for the 7z coder-chain defects fixed in MM45.
//!
//! Every case rebuilds the *exact* folder topology that the frozen MM42 corpus uses, but with
//! self-contained payloads so the coverage runs without external fixture data:
//!
//! * `single_filter`: `LZMA2 -> Delta` (mirrors `delta.7z`).
//! * `multi_stream_dag`: `3xLZMA2 + BCJ2` (mirrors `bcj2_x86.7z`).
//! * `standalone_container`: 7-Zip ZS `BROTLIMT` container (mirrors `brotli.7z`).
//!
//! Each case also asserts a counterexample, so the suite cannot turn green by accident: the
//! bytes produced by the single-coder fast path differ from the expected plaintext, and the
//! ZS container is only stripped when its magic and codec tag are present.
//!
//! The last case covers the malformed-header hardening that the pack-stream mapping relies on:
//! a coder may not inflate the coder index space with an absurd stream count.

use crate::codecs::branch::bcj2::Bcj2Encoder;
use crate::codecs::lzma2::{Fl2CParameter, Fl2CStream, Fl2InBuffer, Fl2OutBuffer};
use crate::crypto::crc32::crc32_fast;
use crate::sevenz::format::{METHOD_BCJ2, METHOD_BROTLI, METHOD_DELTA, METHOD_LZMA2};
use crate::sevenz::header::{
    SevenZCoder, SevenZEntryLocation, SevenZFileMeta, SevenZFolder, SevenZHeaderInfo,
    SevenZSeekIndex,
};

use super::payload::decode_raw_payload_chunked;
use super::stream::extract_entry_bytes_stream;

/// 7-Zip ZS container magic shared by the BROTLI/LZ4 coder families.
const ZS_CONTAINER_MAGIC: u32 = 0x184D2A50;

/// ZS container version emitted for the BROTLI coder.
const ZS_BROTLI_VERSION: u32 = 8;

/// ZS container header layout: `magic(4) | version(4) | size(4) | "BR" 0x01 0x00`.
const ZS_BROTLI_HEADER: [u8; 4] = [0x42, 0x52, 0x01, 0x00];

/// Compresses `src` into a 7z-compatible LZMA2 stream and returns it with its dictionary
/// property byte.
///
/// The streaming compressor is used (not the one-shot context) because 7z stores the LZMA2
/// dictionary property in the folder header and omits it from the payload stream.
fn lzma2_pack(src: &[u8]) -> (Vec<u8>, u8) {
    let mut cstream = Fl2CStream::new().expect("lzma2 stream context");
    cstream
        .set_parameter(Fl2CParameter::CompressionLevel, 6)
        .expect("lzma2 compression level");
    cstream
        .set_parameter(Fl2CParameter::OmitProperties, 1)
        .expect("lzma2 omit properties");
    cstream.init(0).expect("lzma2 stream init");

    let mut packed = Vec::new();
    let mut out_chunk = vec![0u8; 64 * 1024];
    let mut in_buf = Fl2InBuffer {
        src: src.as_ptr() as *const libc::c_void,
        size: src.len(),
        pos: 0,
    };
    while in_buf.pos < in_buf.size {
        let mut out_buf = Fl2OutBuffer {
            dst: out_chunk.as_mut_ptr() as *mut libc::c_void,
            size: out_chunk.len(),
            pos: 0,
        };
        let remaining = cstream
            .compress_chunk(&mut in_buf, &mut out_buf)
            .expect("lzma2 chunk");
        packed.extend_from_slice(&out_chunk[..out_buf.pos]);
        if remaining == 0 && in_buf.pos == 0 {
            break;
        }
    }
    loop {
        let mut out_buf = Fl2OutBuffer {
            dst: out_chunk.as_mut_ptr() as *mut libc::c_void,
            size: out_chunk.len(),
            pos: 0,
        };
        let remaining = cstream.end_stream(&mut out_buf).expect("lzma2 end");
        packed.extend_from_slice(&out_chunk[..out_buf.pos]);
        if remaining == 0 {
            break;
        }
    }
    let prop = cstream.dict_property();
    (packed, prop)
}

/// Builds the single-folder archive metadata and seek index for one plaintext entry.
fn single_entry_case(
    folder: SevenZFolder,
    payload: Vec<u8>,
    pack_sizes: Vec<u64>,
    rel_path: &str,
    plaintext: &[u8],
    primary_method_id: u64,
) -> (SevenZHeaderInfo, SevenZSeekIndex) {
    let crc = crc32_fast(0, plaintext);
    let info = SevenZHeaderInfo {
        payload_offset: 0,
        payload_len: payload.len(),
        pack_sizes,
        folders: vec![folder],
        stream_sizes: vec![plaintext.len() as u64],
        stream_crcs: vec![crc],
        files: vec![SevenZFileMeta {
            rel_path: rel_path.to_string(),
            is_directory: false,
            is_empty_stream: false,
            mtime_epoch_secs: None,
            mode: 0o644,
        }],
        primary_method_id,
        coder_props: Vec::new(),
        is_encrypted: false,
        aes_salt: [0u8; 16],
        aes_salt_len: 0,
        aes_iv: [0u8; 16],
        aes_iv_len: 0,
        aes_num_cycles_power: 0,
    };
    let seek_index = SevenZSeekIndex {
        entries: vec![SevenZEntryLocation {
            file_index: 0,
            rel_path: rel_path.to_string(),
            is_directory: false,
            is_empty_stream: false,
            folder_index: Some(0),
            stream_index: Some(0),
            offset_in_folder: 0,
            uncompressed_size: plaintext.len() as u64,
            crc: Some(crc),
        }],
    };
    (info, seek_index)
}

/// Encodes `plaintext` with the 7z Delta filter (`distance = 1`).
fn delta_encode(plaintext: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(plaintext.len());
    for (index, byte) in plaintext.iter().enumerate() {
        let previous = if index == 0 { 0 } else { plaintext[index - 1] };
        encoded.push(byte.wrapping_sub(previous));
    }
    encoded
}

/// Wraps a raw brotli stream into the 7-Zip ZS `BROTLIMT` container.
fn wrap_zs_brotli(brotli_stream: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(16 + brotli_stream.len());
    payload.extend_from_slice(&ZS_CONTAINER_MAGIC.to_le_bytes());
    payload.extend_from_slice(&ZS_BROTLI_VERSION.to_le_bytes());
    payload.extend_from_slice(&(brotli_stream.len() as u32).to_le_bytes());
    payload.extend_from_slice(&ZS_BROTLI_HEADER);
    payload.extend_from_slice(brotli_stream);
    payload
}

/// Compresses `plaintext` into a bare brotli stream (no 7z container).
fn brotli_compress(plaintext: &[u8]) -> Vec<u8> {
    let mut compressed = Vec::new();
    {
        let mut writer = brotli::CompressorWriter::new(&mut compressed, 4096, 6, 22);
        std::io::Write::write_all(&mut writer, plaintext).expect("brotli compress");
    }
    compressed
}

/// `LZMA2 -> Delta` folder: the single-filter shape of `delta.7z`.
///
/// The fast path decodes the first non-AES coder only and never applies the Delta filter, so it
/// yields the pre-filter bytes. The assertion therefore proves the folder was routed through the
/// coder-chain pipeline.
#[test]
fn test_single_filter_folder_applies_delta_after_lzma2() {
    let plaintext = b"aaaabbbbcccc";
    let delta_bytes = delta_encode(plaintext);
    let (packed, prop) = lzma2_pack(&delta_bytes);

    let folder = SevenZFolder {
        coders: vec![
            SevenZCoder {
                method_id: METHOD_LZMA2,
                num_in_streams: 1,
                num_out_streams: 1,
                properties: vec![prop],
            },
            SevenZCoder {
                method_id: METHOD_DELTA,
                num_in_streams: 1,
                num_out_streams: 1,
                properties: vec![0],
            },
        ],
        bind_pairs: vec![(1, 0)],
        packed_streams: Vec::new(),
        unpack_sizes: vec![plaintext.len() as u64, plaintext.len() as u64],
        crc: None,
        num_unpack_streams: 1,
        packed_offset: 0,
        packed_len: packed.len(),
    };
    let (info, seek_index) = single_entry_case(
        folder,
        packed.clone(),
        vec![packed.len() as u64],
        "delta.txt",
        plaintext,
        METHOD_DELTA,
    );

    let extracted = extract_entry_bytes_stream(&packed, &info, &seek_index, 0, None)
        .expect("delta folder must decode through the coder-chain pipeline");
    assert_eq!(extracted, plaintext);

    // Counterexample: decoding the same pack stream with the single LZMA2 coder yields the
    // pre-filter bytes, so the assertion above cannot be satisfied by the fast path.
    let mut prefilter = Vec::new();
    decode_raw_payload_chunked(
        &packed,
        METHOD_LZMA2,
        &[prop],
        plaintext.len() as u64,
        1,
        |chunk| {
            prefilter.extend_from_slice(chunk);
            Ok(())
        },
    )
    .expect("lzma2-only decode");
    assert_eq!(prefilter, delta_bytes);
    assert_ne!(prefilter, plaintext.to_vec());
}

/// `3xLZMA2 + BCJ2` folder: the multi-stream DAG shape of `bcj2_x86.7z`.
///
/// The folder carries four packed streams (main, rc, call, jump) bound into a four-input BCJ2
/// coder. The single-coder fast path cannot represent that topology at all.
#[test]
fn test_multi_stream_bcj2_folder_runs_through_dag_pipeline() {
    let mut source = Vec::new();
    for step in 0..48u32 {
        source.extend_from_slice(&[0x55, 0x48, 0x89, 0xE5]);
        source.push(0xE8);
        source.extend_from_slice(&(step as i32).to_le_bytes());
        source.push(0xE9);
        source.extend_from_slice(&(step as i32).to_le_bytes());
        source.extend_from_slice(&[0x5D, 0xC3]);
    }

    let streams = Bcj2Encoder::new().encode(&source);
    assert!(
        !streams.call.is_empty() && !streams.jump.is_empty() && !streams.rc.is_empty(),
        "fixture must exercise the call/jump/rc channels"
    );

    let (main_packed, prop) = lzma2_pack(&streams.main);
    let (call_packed, call_prop) = lzma2_pack(&streams.call);
    let (jump_packed, jump_prop) = lzma2_pack(&streams.jump);
    assert_eq!(prop, call_prop);
    assert_eq!(prop, jump_prop);

    // Pack order mirrors the frozen corpus: main, rc, call, jump.
    let mut payload = Vec::new();
    payload.extend_from_slice(&main_packed);
    payload.extend_from_slice(&streams.rc);
    payload.extend_from_slice(&call_packed);
    payload.extend_from_slice(&jump_packed);
    let pack_sizes = vec![
        main_packed.len() as u64,
        streams.rc.len() as u64,
        call_packed.len() as u64,
        jump_packed.len() as u64,
    ];

    let folder = SevenZFolder {
        coders: vec![
            SevenZCoder {
                method_id: METHOD_LZMA2,
                num_in_streams: 1,
                num_out_streams: 1,
                properties: vec![prop],
            },
            SevenZCoder {
                method_id: METHOD_LZMA2,
                num_in_streams: 1,
                num_out_streams: 1,
                properties: vec![prop],
            },
            SevenZCoder {
                method_id: METHOD_LZMA2,
                num_in_streams: 1,
                num_out_streams: 1,
                properties: vec![prop],
            },
            SevenZCoder {
                method_id: METHOD_BCJ2,
                num_in_streams: 4,
                num_out_streams: 1,
                properties: Vec::new(),
            },
        ],
        bind_pairs: vec![(5, 0), (4, 1), (3, 2)],
        packed_streams: vec![2, 6, 1, 0],
        unpack_sizes: vec![
            streams.jump.len() as u64,
            streams.call.len() as u64,
            streams.main.len() as u64,
            source.len() as u64,
        ],
        crc: None,
        num_unpack_streams: 1,
        packed_offset: 0,
        packed_len: payload.len(),
    };
    let (info, seek_index) = single_entry_case(
        folder,
        payload.clone(),
        pack_sizes,
        "synthetic/bin/branch.bin",
        &source,
        METHOD_BCJ2,
    );

    let extracted = extract_entry_bytes_stream(&payload, &info, &seek_index, 0, None)
        .expect("bcj2 folder must decode through the DAG pipeline");
    assert_eq!(extracted, source);

    // Counterexample: the BCJ2 channels are not a plain concatenation of the main stream, so a
    // decoder that ignores the bind graph cannot reproduce the source bytes.
    assert_ne!(streams.main, source);
}

/// BROTLI folder wrapped in the 7-Zip ZS container: the standalone-container shape of
/// `brotli.7z`.
#[test]
fn test_brotli_zs_container_is_stripped_before_decode() {
    let plaintext = b"Owned TTZip codec compliance fixture.";
    let brotli_stream = brotli_compress(plaintext);
    let payload = wrap_zs_brotli(&brotli_stream);

    let folder = SevenZFolder {
        coders: vec![SevenZCoder {
            method_id: METHOD_BROTLI,
            num_in_streams: 1,
            num_out_streams: 1,
            properties: vec![0x01, 0x00, 0x03],
        }],
        bind_pairs: Vec::new(),
        packed_streams: Vec::new(),
        unpack_sizes: vec![plaintext.len() as u64],
        crc: None,
        num_unpack_streams: 1,
        packed_offset: 0,
        packed_len: payload.len(),
    };
    let (info, seek_index) = single_entry_case(
        folder,
        payload.clone(),
        vec![payload.len() as u64],
        "owned.txt",
        plaintext,
        METHOD_BROTLI,
    );

    let extracted = extract_entry_bytes_stream(&payload, &info, &seek_index, 0, None)
        .expect("zs-wrapped brotli folder must decode");
    assert_eq!(extracted, plaintext);
}

/// Counterexample for the container strip: a bare brotli payload must keep decoding, and a
/// payload whose container magic is damaged must be rejected instead of silently accepted.
#[test]
fn test_brotli_container_strip_is_narrow() {
    let plaintext = b"Owned TTZip codec compliance fixture.";
    let brotli_stream = brotli_compress(plaintext);

    let bare_folder = SevenZFolder {
        coders: vec![SevenZCoder {
            method_id: METHOD_BROTLI,
            num_in_streams: 1,
            num_out_streams: 1,
            properties: vec![0x01, 0x00, 0x03],
        }],
        bind_pairs: Vec::new(),
        packed_streams: Vec::new(),
        unpack_sizes: vec![plaintext.len() as u64],
        crc: None,
        num_unpack_streams: 1,
        packed_offset: 0,
        packed_len: brotli_stream.len(),
    };
    let (info, seek_index) = single_entry_case(
        bare_folder,
        brotli_stream.clone(),
        vec![brotli_stream.len() as u64],
        "owned.txt",
        plaintext,
        METHOD_BROTLI,
    );
    let bare = extract_entry_bytes_stream(&brotli_stream, &info, &seek_index, 0, None)
        .expect("bare brotli payload must keep decoding");
    assert_eq!(bare, plaintext);

    let mut damaged = wrap_zs_brotli(&brotli_stream);
    damaged[0] ^= 0xFF;
    let damaged_folder = SevenZFolder {
        coders: vec![SevenZCoder {
            method_id: METHOD_BROTLI,
            num_in_streams: 1,
            num_out_streams: 1,
            properties: vec![0x01, 0x00, 0x03],
        }],
        bind_pairs: Vec::new(),
        packed_streams: Vec::new(),
        unpack_sizes: vec![plaintext.len() as u64],
        crc: None,
        num_unpack_streams: 1,
        packed_offset: 0,
        packed_len: damaged.len(),
    };
    let (damaged_info, damaged_seek) = single_entry_case(
        damaged_folder,
        damaged.clone(),
        vec![damaged.len() as u64],
        "owned.txt",
        plaintext,
        METHOD_BROTLI,
    );
    assert!(
        extract_entry_bytes_stream(&damaged, &damaged_info, &damaged_seek, 0, None).is_err(),
        "a damaged zs container must not decode as a valid brotli stream"
    );
}

/// A coder that claims an absurd input-stream count must be rejected by the header parser.
///
/// The coder stream count drives the bind-pair index space and the folder pack-stream mapping,
/// so an unbounded value would turn both into an unbounded scan or allocation.
#[test]
fn test_header_rejects_absurd_coder_stream_count() {
    let mut header = Vec::new();
    header.push(crate::sevenz::varint::K_UNPACK_INFO);
    header.push(crate::sevenz::varint::K_FOLDER);
    crate::sevenz::varint::encode_7z_varint_vec(1, &mut header); // num_folders
    header.push(0x00); // external = 0
    crate::sevenz::varint::encode_7z_varint_vec(1, &mut header); // num_coders
    header.push(0x11); // flags: one-byte method id plus explicit stream counts
    header.push(METHOD_LZMA2 as u8);
    crate::sevenz::varint::encode_7z_varint_vec(u64::MAX, &mut header); // num_in_streams
    crate::sevenz::varint::encode_7z_varint_vec(1, &mut header); // num_out_streams
    header.push(crate::sevenz::varint::K_END); // end of unpack info
    header.push(crate::sevenz::varint::K_END); // end of header

    let mut info = SevenZHeaderInfo::default();
    assert_eq!(
        crate::sevenz::header::parse_7z_header_stream(&header, &mut info),
        Err(crate::types::TTZipStatus::ErrCorruptHeader)
    );
}
