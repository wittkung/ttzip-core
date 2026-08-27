// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Native 7-Zip (7z) Engine module.
//!
//! Provides zero-copy 7z header parsing, Solid stream selective decoding,
//! ARM64 8-way NEON AES-256-CBC hardware decryption with SHA-256 KDF,
//! and multi-threaded Fast-LZMA2 solid archive creation.

pub mod decoder;
pub mod format;
pub mod header;
pub mod writer;

pub use decoder::{decode_7z_solid_payload, extract_entry_bytes_stream, SevenZArchive};
pub use format::{
    read_varint, write_varint, SevenZSignatureHeader, K_CODERS_UNPACK_SIZE, K_CRC, K_EMPTY_STREAM,
    K_END, K_ENCODED_HEADER, K_FILES_INFO, K_FOLDER, K_HEADER, K_MAIN_STREAMS_INFO, K_NAME,
    K_NUM_UNPACK_STREAM, K_PACK_INFO, K_SIZE, K_SUB_STREAMS_INFO, K_UNPACK_INFO, K_WIN_ATTRIBUTES,
    METHOD_AES, METHOD_COPY, METHOD_DEFLATE, METHOD_LZMA, METHOD_LZMA2, SEVENZ_SIGNATURE,
};
pub use header::{
    parse_7z_metadata, SevenZCoder, SevenZEntryLocation, SevenZFileMeta, SevenZFolder,
    SevenZHeaderInfo, SevenZSeekIndex,
};
pub use writer::{
    build_7z_metadata_header, create_7z_archive, create_7z_solid_archive_bytes,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TTZipStatus;
    use crate::zip::writer::ZipInputItem;

    #[test]
    fn test_7z_solid_roundtrip_lzma2_and_store() {
        let items = vec![
            ZipInputItem {
                rel_path: "doc.txt".to_string(),
                data: b"Document content for 7z native Rust test.".to_vec(),
                mtime_epoch_secs: 1700000000,
                mode: 0o644,
                is_directory: false,
            },
            ZipInputItem {
                rel_path: "assets/".to_string(),
                data: Vec::new(),
                mtime_epoch_secs: 1700000000,
                mode: 0o755,
                is_directory: true,
            },
            ZipInputItem {
                rel_path: "assets/data.bin".to_string(),
                data: vec![0x33u8; 8192],
                mtime_epoch_secs: 1700000000,
                mode: 0o644,
                is_directory: false,
            },
        ];

        // Test LZMA2 level 3
        let archive_bytes = create_7z_solid_archive_bytes(&items, 3, 2).expect("7z creation failed");
        assert!(!archive_bytes.is_empty());

        let archive = SevenZArchive::open_slice(&archive_bytes).expect("7z open failed");
        assert_eq!(archive.len(), 3);

        let f0 = archive.extract_entry_bytes_stream(0, None).expect("extract f0 failed");
        assert_eq!(f0, b"Document content for 7z native Rust test.");

        let f1 = archive.extract_entry_bytes_stream(1, None).expect("extract f1 failed");
        assert!(f1.is_empty());

        let f2 = archive.extract_entry_bytes_stream(2, None).expect("extract f2 failed");
        assert_eq!(f2, vec![0x33u8; 8192]);

        // Test seek index lookup
        let loc = archive.seek_index().get_by_path("assets/data.bin").expect("find f2");
        assert_eq!(loc.file_index, 2);
        assert_eq!(loc.uncompressed_size, 8192);
    }

    #[test]
    fn test_7z_multi_folder_parsing_and_seek_index() {
        use super::header::stream::parse_7z_header_stream;

        let mut h = Vec::new();
        h.push(K_HEADER);
        h.push(K_MAIN_STREAMS_INFO);

        // PackInfo
        h.push(K_PACK_INFO);
        write_varint(0, &mut h); // packPos
        write_varint(2, &mut h); // numPackStreams = 2
        h.push(K_SIZE);
        write_varint(300, &mut h); // pack size 0
        write_varint(250, &mut h); // pack size 1
        h.push(K_END);

        // UnpackInfo
        h.push(K_UNPACK_INFO);
        h.push(K_FOLDER);
        write_varint(2, &mut h); // numFolders = 2
        h.push(0); // external = 0

        // Folder 0: 1 Coder (METHOD_COPY, 1 in, 1 out)
        write_varint(1, &mut h); // numCoders = 1
        h.push(0x01); // method size 1
        h.push(0x00); // METHOD_COPY

        // Folder 1: 1 Coder (METHOD_LZMA2, 1 in, 1 out, props: [20])
        write_varint(1, &mut h); // numCoders = 1
        h.push(0x21); // method size 1, props present
        h.push(0x21); // METHOD_LZMA2
        write_varint(1, &mut h); // props size
        h.push(20); // dict prop

        // CodersUnpackSize
        h.push(K_CODERS_UNPACK_SIZE);
        write_varint(300, &mut h); // Folder 0 unpack size
        write_varint(500, &mut h); // Folder 1 unpack size
        h.push(K_END);

        // SubStreamsInfo
        h.push(K_SUB_STREAMS_INFO);
        h.push(K_NUM_UNPACK_STREAM);
        write_varint(2, &mut h); // Folder 0 has 2 unpack streams
        write_varint(1, &mut h); // Folder 1 has 1 unpack stream

        h.push(K_SIZE);
        write_varint(100, &mut h); // Folder 0 explicit stream 0 size = 100 (stream 1 will be 300 - 100 = 200)
        // Folder 1 has 1 stream, so 0 explicit sizes in K_SIZE (stream size is 500)

        h.push(K_END); // end kSubStreamsInfo
        h.push(K_END); // end kMainStreamsInfo

        // FilesInfo
        h.push(K_FILES_INFO);
        write_varint(4, &mut h); // 4 files

        // Empty streams (file 1 is directory)
        h.push(K_EMPTY_STREAM);
        write_varint(1, &mut h);
        h.push(0b01000000); // file 1 is empty stream

        // Name
        h.push(K_NAME);
        let names = ["file0.txt", "assets/", "file2.txt", "file3.txt"];
        let mut names_u16 = Vec::new();
        for name in &names {
            for u in name.encode_utf16() {
                names_u16.extend_from_slice(&u.to_le_bytes());
            }
            names_u16.extend_from_slice(&0u16.to_le_bytes());
        }
        write_varint((1 + names_u16.len()) as u64, &mut h);
        h.push(0); // external
        h.extend_from_slice(&names_u16);

        // WinAttributes
        h.push(K_WIN_ATTRIBUTES);
        write_varint((2 + 4 * 4) as u64, &mut h);
        h.push(1); // allDefined
        h.push(0); // external
        h.extend_from_slice(&0x20u32.to_le_bytes()); // file0
        h.extend_from_slice(&0x10u32.to_le_bytes()); // assets/ (dir)
        h.extend_from_slice(&0x20u32.to_le_bytes()); // file2
        h.extend_from_slice(&0x20u32.to_le_bytes()); // file3

        h.push(K_END); // end kFilesInfo
        h.push(K_END); // end kHeader

        let mut out_info = SevenZHeaderInfo::default();
        parse_7z_header_stream(&h, &mut out_info).expect("parse multi-folder stream failed");

        assert_eq!(out_info.folders.len(), 2);
        assert_eq!(out_info.folders[0].unpack_sizes, vec![300]);
        assert_eq!(out_info.folders[1].unpack_sizes, vec![500]);
        assert_eq!(out_info.folders[0].num_unpack_streams, 2);
        assert_eq!(out_info.folders[1].num_unpack_streams, 1);
        assert_eq!(out_info.stream_sizes, vec![100, 200, 500]);

        let seek_index = SevenZSeekIndex::build(&out_info);
        assert_eq!(seek_index.entries.len(), 4);

        // Entry 0: file0.txt (Folder 0, Stream 0, Offset 0, Size 100)
        let e0 = &seek_index.entries[0];
        assert_eq!(e0.rel_path, "file0.txt");
        assert_eq!(e0.folder_index, Some(0));
        assert_eq!(e0.stream_index, Some(0));
        assert_eq!(e0.offset_in_folder, 0);
        assert_eq!(e0.uncompressed_size, 100);
        assert!(!e0.is_directory);

        // Entry 1: assets/ (directory, no folder/stream)
        let e1 = &seek_index.entries[1];
        assert_eq!(e1.rel_path, "assets/");
        assert!(e1.is_directory);
        assert_eq!(e1.folder_index, None);
        assert_eq!(e1.stream_index, None);
        assert_eq!(e1.uncompressed_size, 0);

        // Entry 2: file2.txt (Folder 0, Stream 1, Offset 100, Size 200)
        let e2 = &seek_index.entries[2];
        assert_eq!(e2.rel_path, "file2.txt");
        assert_eq!(e2.folder_index, Some(0));
        assert_eq!(e2.stream_index, Some(1));
        assert_eq!(e2.offset_in_folder, 100);
        assert_eq!(e2.uncompressed_size, 200);

        // Entry 3: file3.txt (Folder 1, Stream 2, Offset 0, Size 500)
        let e3 = &seek_index.entries[3];
        assert_eq!(e3.rel_path, "file3.txt");
        assert_eq!(e3.folder_index, Some(1));
        assert_eq!(e3.stream_index, Some(2));
        assert_eq!(e3.offset_in_folder, 0);
        assert_eq!(e3.uncompressed_size, 500);
    }

    #[test]
    fn test_7z_unsupported_codec_routing() {
        let mut info = SevenZHeaderInfo::default();
        info.payload_offset = 0;
        info.payload_len = 16;
        info.primary_method_id = 0x999999; // Unknown unsupported method ID
        info.stream_sizes = vec![16];

        let payload = vec![0u8; 16];
        let result = decode_7z_solid_payload(&payload, &info, None, 1);
        assert_eq!(result, Err(TTZipStatus::ErrUnsupportedFeature));
    }

    #[test]
    fn test_7z_multi_folder_full_archive_extraction() {
        use crate::crypto::crc32::crc32_fast;

        let f0_data = b"Hello from Folder 0 stream 0!";
        let f1_data = b"Second stream in Folder 0!";
        let mut folder0_uncomp = Vec::new();
        folder0_uncomp.extend_from_slice(f0_data);
        folder0_uncomp.extend_from_slice(f1_data);

        let f2_data = b"Standalone stream inside Folder 1!";
        let folder1_uncomp = f2_data.to_vec();

        let mut payload = Vec::new();
        payload.extend_from_slice(&folder0_uncomp);
        payload.extend_from_slice(&folder1_uncomp);

        let mut h = Vec::new();
        h.push(K_HEADER);
        h.push(K_MAIN_STREAMS_INFO);

        // PackInfo
        h.push(K_PACK_INFO);
        write_varint(0, &mut h); // packPos
        write_varint(2, &mut h); // numPackStreams = 2
        h.push(K_SIZE);
        write_varint(folder0_uncomp.len() as u64, &mut h);
        write_varint(folder1_uncomp.len() as u64, &mut h);
        h.push(K_END);

        // UnpackInfo
        h.push(K_UNPACK_INFO);
        h.push(K_FOLDER);
        write_varint(2, &mut h); // numFolders = 2
        h.push(0); // external = 0

        // Folder 0: 1 Coder (METHOD_COPY)
        write_varint(1, &mut h);
        h.push(0x01);
        h.push(0x00);

        // Folder 1: 1 Coder (METHOD_COPY)
        write_varint(1, &mut h);
        h.push(0x01);
        h.push(0x00);

        // CodersUnpackSize
        h.push(K_CODERS_UNPACK_SIZE);
        write_varint(folder0_uncomp.len() as u64, &mut h);
        write_varint(folder1_uncomp.len() as u64, &mut h);
        h.push(K_END);

        // SubStreamsInfo
        h.push(K_SUB_STREAMS_INFO);
        h.push(K_NUM_UNPACK_STREAM);
        write_varint(2, &mut h); // Folder 0: 2 streams
        write_varint(1, &mut h); // Folder 1: 1 stream

        h.push(K_SIZE);
        write_varint(f0_data.len() as u64, &mut h); // Folder 0 stream 0 explicit size

        h.push(K_END); // end kSubStreamsInfo
        h.push(K_END); // end kMainStreamsInfo

        // FilesInfo
        h.push(K_FILES_INFO);
        write_varint(3, &mut h); // 3 files

        // Name
        h.push(K_NAME);
        let names = ["f0.txt", "f1.txt", "f2.txt"];
        let mut names_u16 = Vec::new();
        for name in &names {
            for u in name.encode_utf16() {
                names_u16.extend_from_slice(&u.to_le_bytes());
            }
            names_u16.extend_from_slice(&0u16.to_le_bytes());
        }
        write_varint((1 + names_u16.len()) as u64, &mut h);
        h.push(0); // external
        h.extend_from_slice(&names_u16);

        // WinAttributes
        h.push(K_WIN_ATTRIBUTES);
        write_varint((2 + 3 * 4) as u64, &mut h);
        h.push(1); // allDefined
        h.push(0); // external
        h.extend_from_slice(&0x20u32.to_le_bytes());
        h.extend_from_slice(&0x20u32.to_le_bytes());
        h.extend_from_slice(&0x20u32.to_le_bytes());

        h.push(K_END); // end kFilesInfo
        h.push(K_END); // end kHeader

        let sig = SevenZSignatureHeader {
            major_version: 0,
            minor_version: 4,
            start_header_crc: 0,
            next_header_offset: payload.len() as u64,
            next_header_size: h.len() as u64,
            next_header_crc: crc32_fast(0, &h),
        };

        let mut archive_bytes = Vec::new();
        archive_bytes.extend_from_slice(&sig.serialize());
        archive_bytes.extend_from_slice(&payload);
        archive_bytes.extend_from_slice(&h);

        let archive = SevenZArchive::open_slice(&archive_bytes).expect("open multi-folder archive");
        assert_eq!(archive.len(), 3);

        let ext_f0 = archive.extract_entry_bytes_stream(0, None).expect("extract f0");
        assert_eq!(ext_f0, f0_data);

        let ext_f1 = archive.extract_entry_bytes_stream(1, None).expect("extract f1");
        assert_eq!(ext_f1, f1_data);

        let ext_f2 = archive.extract_entry_bytes_stream(2, None).expect("extract f2");
        assert_eq!(ext_f2, f2_data);
    }
}
