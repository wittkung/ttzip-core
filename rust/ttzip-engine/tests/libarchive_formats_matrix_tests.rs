// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 20+ Container Formats Reader/Writer State Machine & Extreme Boundary Matrix Test Suite (Task 16.9).
//!
//! Validates roundtrip packing, unpacking, metadata preservation, and extreme boundary conditions:
//!
//! 1. **TAR Family Matrix**:
//!    - V7 Traditional Unix Tar (numeric octal fields, basic roundtrip).
//!    - POSIX.1-1988 USTAR (prefix/name splitting, permissions, directories, symlinks).
//!    - POSIX.1-2001 PAX (Nanosecond timestamps, UTF-8 extended attributes, Pre-1970 negative timestamps).
//!    - GNU Tar (`././@LongLink` >100 byte long filenames and long hardlinks).
//!
//! 2. **CPIO Family Matrix**:
//!    - POSIX.1 odc (old character format `070707`).
//!    - SVR4 newc (portable ASCII format without CRC `070701`, 4-byte padding alignment).
//!    - SVR4 with CRC (`070702` checksum verification).
//!    - Binary Big-Endian (`0x71C7` 26-byte header stream).
//!    - Binary Little-Endian (`0xC771` stream).
//!
//! 3. **ISO 9660 & UDF Matrix**:
//!    - ISO 9660 Level 1 / 2 / 3 optical disc image.
//!    - Rockridge Extensions (PX, RR, TF, NM, SL symbolic links).
//!    - Joliet Extensions (Unicode UTF-16BE international filenames).
//!    - UDF Volume Recognition Sequence & sniffing.
//!
//! 4. **Apple & BSD Matrix**:
//!    - AppleDouble (`._`) Resource Fork & FinderInfo synthesis and decoding.
//!    - AR BSD Variant (`!<arch>\n` `#1/len` extended naming).
//!    - AR GNU/SVR4 Variant (`//` string table, Debian package members).
//!    - XAR (XML Table of Contents + Compressed Heap Stream).
//!
//! 5. **Special Container Matrix**:
//!    - WARC 1.0/1.1 (Web ARChive headers + HTTP response payloads).
//!    - MTREE (BSD Directory Hierarchy Specification Manifest).
//!    - Microsoft Cabinet (CAB MSCF header & block boundary validation).
//!    - LHA / LZH (`-lh5-` stream detection).

mod formats_matrix_harness;

use formats_matrix_harness::apple_bsd_matrix::*;
use formats_matrix_harness::cpio_matrix::*;
use formats_matrix_harness::iso_matrix::*;
use formats_matrix_harness::special_matrix::*;
use formats_matrix_harness::tar_matrix::*;

// ===========================================================================
// 1. TAR Family Matrix Tests
// ===========================================================================

#[test]
fn test_matrix_tar_v7() {
    run_tar_v7_matrix_test();
}

#[test]
fn test_matrix_tar_ustar() {
    run_tar_ustar_matrix_test();
}

#[test]
fn test_matrix_tar_pax_nanoseconds() {
    run_tar_pax_nanosecond_matrix_test();
}

#[test]
fn test_matrix_tar_pax_utf8_and_xattrs() {
    run_tar_pax_utf8_and_xattrs_matrix_test();
}

#[test]
fn test_matrix_tar_pax_negative_timestamps() {
    run_tar_pax_negative_timestamps_matrix_test();
}

#[test]
fn test_matrix_tar_gnutar_longlinks() {
    run_tar_gnutar_longlink_matrix_test();
}

// ===========================================================================
// 2. CPIO Family Matrix Tests
// ===========================================================================

#[test]
fn test_matrix_cpio_odc() {
    run_cpio_odc_matrix_test();
}

#[test]
fn test_matrix_cpio_newc() {
    run_cpio_newc_matrix_test();
}

#[test]
fn test_matrix_cpio_crc() {
    run_cpio_crc_matrix_test();
}

#[test]
fn test_matrix_cpio_binary_big_endian() {
    run_cpio_binary_be_matrix_test();
}

#[test]
fn test_matrix_cpio_binary_little_endian() {
    run_cpio_binary_le_matrix_test();
}

// ===========================================================================
// 3. ISO 9660 & UDF Matrix Tests
// ===========================================================================

#[test]
fn test_matrix_iso9660_level1() {
    run_iso9660_level1_matrix_test();
}

#[test]
fn test_matrix_iso9660_level2_3() {
    run_iso9660_level2_3_matrix_test();
}

#[test]
fn test_matrix_iso9660_rockridge() {
    run_iso9660_rockridge_matrix_test();
}

#[test]
fn test_matrix_iso9660_joliet_utf16() {
    run_iso9660_joliet_matrix_test();
}

#[test]
fn test_matrix_udf_and_iso_sniffing() {
    run_udf_and_iso_sniffing_matrix_test();
}

// ===========================================================================
// 4. Apple & BSD Matrix Tests
// ===========================================================================

#[test]
fn test_matrix_appledouble_finderinfo() {
    run_appledouble_finderinfo_matrix_test();
}

#[test]
fn test_matrix_ar_bsd_variant() {
    run_ar_bsd_variant_matrix_test();
}

#[test]
fn test_matrix_ar_gnu_svr4_variant() {
    run_ar_gnu_svr4_variant_matrix_test();
}

#[test]
fn test_matrix_xar_xml_toc_compressed_heap() {
    run_xar_matrix_test();
}

// ===========================================================================
// 5. Special Container Matrix Tests
// ===========================================================================

#[test]
fn test_matrix_warc_web_archive() {
    run_warc_matrix_test();
}

#[test]
fn test_matrix_mtree_manifest() {
    run_mtree_matrix_test();
}

#[test]
fn test_matrix_cab_boundary() {
    run_cab_boundary_matrix_test();
}

#[test]
fn test_matrix_lha_lh5_boundary() {
    run_lha_boundary_matrix_test();
}
