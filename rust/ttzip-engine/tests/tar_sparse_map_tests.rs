// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit and integration test suite for GNU Sparse (0.0, 0.1, 1.0)
//! and PAX Sparse 0.1 map parsers, hole calculators, and security defense barriers.

use std::io::Cursor;
use ttzip_engine::tar::codec::numeric_extended_into;
use ttzip_engine::tar::sparse::{
    parse_gnu_sparse_0_x, parse_gnu_sparse_1_0_stream, parse_pax_sparse_0_1, SparseExtent,
    SparseMap, TarSparseError,
};
use ttzip_engine::tar::types::{GnuExtSparseHeader, TarEntryType, BLOCK_SIZE};
use ttzip_engine::tar::TarHeader;

/// Helper to construct a standard GNU Sparse 0.0 TarHeader.
fn make_gnu_sparse_header(real_size: u64, is_extended: bool, extents: &[(u64, u64)]) -> TarHeader {
    let mut header = TarHeader::new();
    header.set_gnu_magic();
    header.set_entry_type(TarEntryType::GNUSparse);
    header.set_name("sparse_file.img");

    let gnu = header.as_gnu_header_mut();
    numeric_extended_into(&mut gnu.realsize, real_size);
    gnu.isextended = if is_extended { 1 } else { 0 };

    for (i, &(offset, numbytes)) in extents.iter().take(4).enumerate() {
        numeric_extended_into(&mut gnu.sparse[i].offset, offset);
        numeric_extended_into(&mut gnu.sparse[i].numbytes, numbytes);
    }

    header.update_checksum();
    header
}

/// Helper to construct a GnuExtSparseHeader sector.
fn make_gnu_ext_sparse_header(is_extended: bool, extents: &[(u64, u64)]) -> GnuExtSparseHeader {
    let mut ext = GnuExtSparseHeader {
        sparse: Default::default(),
        isextended: if is_extended { 1 } else { 0 },
        pad: [0; 7],
    };

    for (i, &(offset, numbytes)) in extents.iter().take(21).enumerate() {
        numeric_extended_into(&mut ext.sparse[i].offset, offset);
        numeric_extended_into(&mut ext.sparse[i].numbytes, numbytes);
    }

    ext
}

#[test]
fn test_gnu_sparse_0_0_basic_parsing() {
    let extents = [(0u64, 4096u64), (1044480u64, 4096u64)];
    let header = make_gnu_sparse_header(1_048_576, false, &extents);

    let map = parse_gnu_sparse_0_x(&header, &[]).expect("failed to parse GNU 0.0 sparse map");

    assert_eq!(map.real_size, 1_048_576);
    assert_eq!(map.extents.len(), 2);
    assert_eq!(map.extents[0], SparseExtent::new(0, 4096));
    assert_eq!(map.extents[1], SparseExtent::new(1044480, 4096));

    assert_eq!(map.total_data_bytes(), 8192);
    assert_eq!(map.total_hole_bytes(), 1_040_384);
    assert!(map.has_holes());

    let holes = map.calculate_hole_ranges();
    assert_eq!(holes.len(), 1);
    assert_eq!(holes[0], (4096, 1040384));
}

#[test]
fn test_gnu_sparse_0_1_multi_sector_extension() {
    // 50 extents total across primary header + 3 extended sectors
    let mut all_extents = Vec::new();
    let extent_size = 4096u64;
    let stride = 65536u64; // 4KB data followed by 60KB hole

    for i in 0..50u64 {
        all_extents.push((i * stride, extent_size));
    }

    let real_size = 50 * stride;
    let header = make_gnu_sparse_header(real_size, true, &all_extents[0..4]);

    let ext1 = make_gnu_ext_sparse_header(true, &all_extents[4..25]);
    let ext2 = make_gnu_ext_sparse_header(true, &all_extents[25..46]);
    let ext3 = make_gnu_ext_sparse_header(false, &all_extents[46..50]);

    let map = parse_gnu_sparse_0_x(&header, &[ext1, ext2, ext3])
        .expect("failed to parse GNU 0.1 multi-sector sparse map");

    assert_eq!(map.real_size, real_size);
    assert_eq!(map.extents.len(), 50);
    assert_eq!(map.total_data_bytes(), 50 * 4096);

    for (i, ext) in map.extents.iter().enumerate() {
        assert_eq!(ext.offset, (i as u64) * stride);
        assert_eq!(ext.numbytes, extent_size);
    }

    let holes = map.calculate_hole_ranges();
    assert_eq!(holes.len(), 50);
    for (i, &(hole_offset, hole_len)) in holes.iter().enumerate() {
        assert_eq!(hole_offset, (i as u64) * stride + 4096);
        assert_eq!(hole_len, 65536 - 4096);
    }
}

#[test]
fn test_gnu_sparse_0_1_missing_extended_header_defense() {
    let header = make_gnu_sparse_header(10000, true, &[(0, 1000)]);
    let err = parse_gnu_sparse_0_x(&header, &[]).unwrap_err();
    assert_eq!(err, TarSparseError::MissingExtendedHeader);
}

#[test]
fn test_pax_sparse_0_1_valid_parsing() {
    let pax_map = "0,4096,1048576,8192,2097152,4096";
    let real_size = 4_194_304u64;

    let map = parse_pax_sparse_0_1(pax_map, real_size).expect("failed to parse PAX 0.1 map");

    assert_eq!(map.real_size, real_size);
    assert_eq!(map.extents.len(), 3);
    assert_eq!(map.extents[0], SparseExtent::new(0, 4096));
    assert_eq!(map.extents[1], SparseExtent::new(1048576, 8192));
    assert_eq!(map.extents[2], SparseExtent::new(2097152, 4096));

    let holes = map.calculate_hole_ranges();
    assert_eq!(
        holes,
        vec![
            (4096, 1048576 - 4096),
            (1048576 + 8192, 2097152 - (1048576 + 8192)),
            (2097152 + 4096, 4194304 - (2097152 + 4096)),
        ]
    );
}

#[test]
fn test_pax_sparse_0_1_whitespace_and_empty() {
    let pax_map = "  0 ,  4096 , 1048576 , 8192  ";
    let map = parse_pax_sparse_0_1(pax_map, 2_000_000).expect("failed with whitespace");
    assert_eq!(map.extents.len(), 2);

    let empty_map = parse_pax_sparse_0_1("", 1_000_000).expect("failed empty map");
    assert_eq!(empty_map.extents.len(), 0);
    assert_eq!(empty_map.calculate_hole_ranges(), vec![(0, 1_000_000)]);
}

#[test]
fn test_pax_sparse_0_1_malformed_defense() {
    // Odd count
    let err = parse_pax_sparse_0_1("0,4096,1048576", 2_000_000).unwrap_err();
    assert!(matches!(err, TarSparseError::InvalidPaxMap(_)));

    // Non-numeric
    let err2 = parse_pax_sparse_0_1("0,not_a_number", 2_000_000).unwrap_err();
    assert!(matches!(err2, TarSparseError::InvalidPaxMap(_)));

    // Negative number
    let err3 = parse_pax_sparse_0_1("0,-500", 2_000_000).unwrap_err();
    assert!(matches!(err3, TarSparseError::InvalidPaxMap(_)));
}

#[test]
fn test_gnu_sparse_1_0_stream_parsing_and_sector_alignment() {
    // Construct GNU sparse 1.0 stream text header
    let text_header = "3\n0\n4096\n1048576\n8192\n10485760\n16384\n";
    let mut stream_bytes = text_header.as_bytes().to_vec();

    // Pad stream_bytes to 512-byte boundary
    let pad_len = BLOCK_SIZE - (stream_bytes.len() % BLOCK_SIZE);
    stream_bytes.resize(stream_bytes.len() + pad_len, 0);

    // Append some payload bytes after header
    stream_bytes.extend_from_slice(b"PAYLOAD_DATA_BEGIN");

    let real_size = 20_971_520u64;
    let mut cursor = Cursor::new(&stream_bytes);

    let (map, consumed) = parse_gnu_sparse_1_0_stream(&mut cursor, real_size)
        .expect("failed to parse GNU 1.0 stream map");

    assert_eq!(consumed, BLOCK_SIZE); // Exact 1 sector consumed
    assert_eq!(cursor.position(), BLOCK_SIZE as u64);

    assert_eq!(map.real_size, real_size);
    assert_eq!(map.extents.len(), 3);
    assert_eq!(map.extents[0], SparseExtent::new(0, 4096));
    assert_eq!(map.extents[1], SparseExtent::new(1048576, 8192));
    assert_eq!(map.extents[2], SparseExtent::new(10485760, 16384));

    let holes = map.calculate_hole_ranges();
    assert_eq!(holes.len(), 3);
    assert_eq!(holes[0], (4096, 1048576 - 4096));
    assert_eq!(holes[1], (1048576 + 8192, 10485760 - (1048576 + 8192)));
    assert_eq!(
        holes[2],
        (10485760 + 16384, 20971520 - (10485760 + 16384))
    );
}

#[test]
fn test_gnu_sparse_1_0_multi_sector_stream_header() {
    // 50 entries will exceed 512 bytes and span into multi-sector header
    let mut text_header = String::from("50\n");
    let stride = 100_000u64;
    let block_len = 8192u64;

    for i in 0..50u64 {
        text_header.push_str(&format!("{}\n{}\n", i * stride, block_len));
    }

    let mut stream_bytes = text_header.into_bytes();
    let rem = stream_bytes.len() % BLOCK_SIZE;
    if rem != 0 {
        stream_bytes.resize(stream_bytes.len() + (BLOCK_SIZE - rem), 0);
    }
    let expected_sectors_bytes = stream_bytes.len();

    let real_size = 50 * stride;
    let mut cursor = Cursor::new(&stream_bytes);

    let (map, consumed) = parse_gnu_sparse_1_0_stream(&mut cursor, real_size)
        .expect("failed multi-sector GNU 1.0 stream map");

    assert_eq!(consumed, expected_sectors_bytes);
    assert_eq!(map.extents.len(), 50);
}

#[test]
fn test_gnu_sparse_1_0_corrupted_stream_defense() {
    // Truncated stream
    let mut cursor1 = Cursor::new(b"5\n0\n4096\n");
    let err1 = parse_gnu_sparse_1_0_stream(&mut cursor1, 10000).unwrap_err();
    assert!(matches!(err1, TarSparseError::InvalidStreamMap(_)));

    // Non-numeric count
    let mut cursor2 = Cursor::new(vec![b'x'; 512]);
    let err2 = parse_gnu_sparse_1_0_stream(&mut cursor2, 10000).unwrap_err();
    assert!(matches!(err2, TarSparseError::InvalidStreamMap(_)));
}

#[test]
fn test_sparse_map_validation_defenses() {
    // Non-sparse entry flag
    let mut normal_header = TarHeader::new();
    normal_header.set_entry_type(TarEntryType::Regular);
    let err = parse_gnu_sparse_0_x(&normal_header, &[]).unwrap_err();
    assert_eq!(err, TarSparseError::NotSparseEntry);

    // Extent exceeds real size
    let map_exceed = SparseMap::new(1000, vec![SparseExtent::new(500, 600)]);
    let err_exceed = map_exceed.validate_sparse_map().unwrap_err();
    assert!(matches!(err_exceed, TarSparseError::ExceedsRealSize { .. }));

    // Overlapping extents
    let map_overlap = SparseMap::new(
        2000,
        vec![SparseExtent::new(0, 500), SparseExtent::new(400, 500)],
    );
    let err_overlap = map_overlap.validate_sparse_map().unwrap_err();
    assert!(matches!(
        err_overlap,
        TarSparseError::OverlappingExtents { .. }
    ));

    // Disordered extents
    let map_disordered = SparseMap::new(
        2000,
        vec![SparseExtent::new(1000, 200), SparseExtent::new(500, 200)],
    );
    let err_disordered = map_disordered.validate_sparse_map().unwrap_err();
    assert!(matches!(
        err_disordered,
        TarSparseError::DisorderedExtents { .. }
    ));

    // Integer overflow extent
    let map_overflow = SparseMap::new(
        u64::MAX,
        vec![SparseExtent::new(u64::MAX - 10, 20)],
    );
    let err_overflow = map_overflow.validate_sparse_map().unwrap_err();
    assert_eq!(err_overflow, TarSparseError::IntegerOverflow);
}

#[test]
fn test_sparse_map_zero_size_and_dense_file() {
    // 0-size file
    let map_zero = SparseMap::new(0, vec![]);
    assert!(map_zero.validate_sparse_map().is_ok());
    assert!(!map_zero.has_holes());
    assert_eq!(map_zero.calculate_hole_ranges(), vec![]);

    // Dense file (1 extent covering entire size)
    let map_dense = SparseMap::new(1024, vec![SparseExtent::new(0, 1024)]);
    assert!(map_dense.validate_sparse_map().is_ok());
    assert!(!map_dense.has_holes());
    assert_eq!(map_dense.calculate_hole_ranges(), vec![]);
    assert_eq!(map_dense.total_data_bytes(), 1024);
    assert_eq!(map_dense.total_hole_bytes(), 0);
}
