// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! CPIO Family Format Matrix (POSIX.1 odc, SVR4 newc, CRC, Binary BE, Binary LE).

use super::{
    assert_roundtrip_match, compute_sha256, read_archive_buffer, write_archive_buffer, SyntheticEntry,
    VerifyPolicy,
};
use ttzip_engine::ffi::archive_ffi::sys::*;

/// 1. POSIX.1 odc old character format (070707).
pub fn run_cpio_odc_matrix_test() {
    let entries = vec![
        SyntheticEntry::file("cpio_odc/doc.txt", b"POSIX.1 odc format text payload".to_vec())
            .with_perm(0o644)
            .with_mtime(1_600_000_000, 0),
        SyntheticEntry::file("cpio_odc/data.bin", vec![0x33; 1024])
            .with_perm(0o755)
            .with_mtime(1_600_000_010, 0),
        SyntheticEntry::dir("cpio_odc/sub_dir/")
            .with_perm(0o755)
            .with_mtime(1_600_000_020, 0),
    ];

    let bytes = write_archive_buffer(&entries, |a| unsafe {
        let rc = archive_write_set_format_cpio_odc(a);
        if rc != 0 {
            Err("archive_write_set_format_cpio_odc failed".to_string())
        } else {
            Ok(())
        }
    })
    .expect("Failed to write CPIO odc archive");

    assert!(!bytes.is_empty());
    // Verify CPIO odc magic "070707"
    assert!(bytes.starts_with(b"070707"), "CPIO odc must start with 070707");

    let extracted = read_archive_buffer(&bytes).expect("Failed to read CPIO odc archive");
    assert_roundtrip_match(&entries, &extracted, &VerifyPolicy::default());
}

/// 2. SVR4 Portable ASCII Format without CRC (070701 newc).
pub fn run_cpio_newc_matrix_test() {
    let entries = vec![
        SyntheticEntry::file("cpio_newc/kernel.img", vec![0x90; 4096])
            .with_perm(0o644)
            .with_mtime(1_680_000_000, 0),
        SyntheticEntry::symlink("cpio_newc/vmlinuz_symlink", "kernel.img")
            .with_mtime(1_680_000_001, 0),
        SyntheticEntry::file("cpio_newc/initrd.config", b"MODULES=most\nCOMPRESS=zstd\n".to_vec())
            .with_perm(0o600)
            .with_mtime(1_680_000_002, 0),
    ];

    let bytes = write_archive_buffer(&entries, |a| unsafe {
        let rc = archive_write_set_format_cpio_newc(a);
        if rc != 0 {
            Err("archive_write_set_format_cpio_newc failed".to_string())
        } else {
            Ok(())
        }
    })
    .expect("Failed to write CPIO newc archive");

    assert!(!bytes.is_empty());
    // Verify CPIO newc magic "070701"
    assert!(bytes.starts_with(b"070701"), "CPIO newc must start with 070701");

    let extracted = read_archive_buffer(&bytes).expect("Failed to read CPIO newc archive");
    assert_roundtrip_match(&entries, &extracted, &VerifyPolicy::default());
}

/// 3. SVR4 Portable ASCII Format with CRC (070702).
pub fn run_cpio_crc_matrix_test() {
    let payload1 = vec![0x5A; 8192];
    let payload2 = b"checksum verification data".to_vec();

    let synth_records = vec![
        ("cpio_crc/verified_payload.dat", payload1.as_slice(), 0o100644, 1_690_000_000),
        ("cpio_crc/checksums.sha256", payload2.as_slice(), 0o100644, 1_690_000_010),
    ];

    let bytes = synthesize_svr4_cpio(&synth_records, true);
    assert!(bytes.starts_with(b"070702"), "CPIO CRC must start with 070702");

    let extracted = read_archive_buffer(&bytes).expect("Failed to read CPIO CRC archive");
    assert_eq!(extracted.len(), 2);
    assert_eq!(extracted[0].path, "cpio_crc/verified_payload.dat");
    assert_eq!(extracted[0].data, payload1);
    assert_eq!(extracted[0].sha256, compute_sha256(&payload1));
    assert_eq!(extracted[1].path, "cpio_crc/checksums.sha256");
    assert_eq!(extracted[1].data, payload2);
    assert_eq!(extracted[1].sha256, compute_sha256(&payload2));
}

/// Synthesizes an SVR4 ASCII CPIO byte stream (070701 newc or 070702 crc).
fn synthesize_svr4_cpio(
    entries: &[(&str, &[u8], u32, i64)],
    is_crc: bool,
) -> Vec<u8> {
    let mut buffer = Vec::new();
    let magic = if is_crc { "070702" } else { "070701" };

    for (dev_ino, (name, data, mode, mtime)) in (1u32..).zip(entries.iter()) {
        let name_bytes = name.as_bytes();
        let name_len_with_nul = (name_bytes.len() + 1) as u32;
        let file_size = data.len() as u32;
        let crc: u32 = data.iter().map(|&b| b as u32).sum();

        let header = format!(
            "{}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}",
            magic,
            dev_ino, // c_ino
            mode,    // c_mode
            1000,    // c_uid
            1000,    // c_gid
            1,       // c_nlink
            mtime,   // c_mtime
            file_size,
            dev_ino, // c_maj
            dev_ino, // c_min
            0,       // c_rmaj
            0,       // c_rmin
            name_len_with_nul,
            crc,     // c_chksum
        );

        buffer.extend_from_slice(header.as_bytes());
        buffer.extend_from_slice(name_bytes);
        buffer.push(0); // NUL terminator
        while buffer.len() % 4 != 0 {
            buffer.push(0);
        }

        buffer.extend_from_slice(data);
        while buffer.len() % 4 != 0 {
            buffer.push(0);
        }
    }

    // TRAILER!!! record
    let trailer_name = b"TRAILER!!!\0";
    let trailer_header = format!(
        "{}0000000000000000000000000000000000000001000000000000000000000000000000000000000000000000{:08X}00000000",
        magic,
        trailer_name.len()
    );
    buffer.extend_from_slice(trailer_header.as_bytes());
    buffer.extend_from_slice(trailer_name);
    while buffer.len() % 4 != 0 {
        buffer.push(0);
    }

    // Pad total archive to 512 bytes
    let remainder = buffer.len() % 512;
    if remainder != 0 {
        buffer.resize(buffer.len() + (512 - remainder), 0);
    }

    buffer
}

/// Helper to synthesize a binary CPIO stream (Big-Endian `0x71C7` or Little-Endian `0xC771`).
fn synthesize_binary_cpio(
    entries: &[(&str, &[u8], u32, i64)],
    is_big_endian: bool,
) -> Vec<u8> {
    let mut buffer = Vec::new();
    let magic: u16 = 0x71C7;

    let write_u16 = |buf: &mut Vec<u8>, val: u16| {
        if is_big_endian {
            buf.extend_from_slice(&val.to_be_bytes());
        } else {
            buf.extend_from_slice(&val.to_le_bytes());
        }
    };

    let write_u32_pair = |buf: &mut Vec<u8>, val: u32| {
        let hi = (val >> 16) as u16;
        let lo = (val & 0xFFFF) as u16;
        if is_big_endian {
            buf.extend_from_slice(&hi.to_be_bytes());
            buf.extend_from_slice(&lo.to_be_bytes());
        } else {
            buf.extend_from_slice(&hi.to_le_bytes());
            buf.extend_from_slice(&lo.to_le_bytes());
        }
    };

    for (dev_ino, (name, data, mode, mtime)) in (1u16..).zip(entries.iter()) {
        let name_bytes = name.as_bytes();
        let name_len_with_nul = (name_bytes.len() + 1) as u16;
        let file_size = data.len() as u32;

        write_u16(&mut buffer, magic);
        write_u16(&mut buffer, dev_ino); // h_dev
        write_u16(&mut buffer, dev_ino); // h_ino
        write_u16(&mut buffer, *mode as u16); // h_mode
        write_u16(&mut buffer, 1000); // h_uid
        write_u16(&mut buffer, 1000); // h_gid
        write_u16(&mut buffer, 1); // h_nlink
        write_u16(&mut buffer, 0); // h_rdev
        write_u32_pair(&mut buffer, *mtime as u32); // h_mtime
        write_u16(&mut buffer, name_len_with_nul); // h_namesize
        write_u32_pair(&mut buffer, file_size); // h_filesize

        // Write name + NUL
        buffer.extend_from_slice(name_bytes);
        buffer.push(0);
        // Pad name to 2-byte boundary
        if (name_bytes.len() + 1) % 2 != 0 {
            buffer.push(0);
        }

        // Write data
        buffer.extend_from_slice(data);
        // Pad data to 2-byte boundary
        if data.len() % 2 != 0 {
            buffer.push(0);
        }
    }

    // Write TRAILER!!! entry
    let trailer_name = b"TRAILER!!!\0";
    write_u16(&mut buffer, magic);
    write_u16(&mut buffer, 0);
    write_u16(&mut buffer, 0);
    write_u16(&mut buffer, 0);
    write_u16(&mut buffer, 0);
    write_u16(&mut buffer, 0);
    write_u16(&mut buffer, 1);
    write_u16(&mut buffer, 0);
    write_u32_pair(&mut buffer, 0);
    write_u16(&mut buffer, trailer_name.len() as u16);
    write_u32_pair(&mut buffer, 0);

    buffer.extend_from_slice(trailer_name);
    if !trailer_name.len().is_multiple_of(2) {
        buffer.push(0);
    }

    // Pad total archive to 512 bytes
    let remainder = buffer.len() % 512;
    if remainder != 0 {
        buffer.resize(buffer.len() + (512 - remainder), 0);
    }

    buffer
}

/// 4. Binary Big-Endian CPIO (0x71C7).
pub fn run_cpio_binary_be_matrix_test() {
    let payload1 = b"Binary Big Endian CPIO Entry 1";
    let payload2 = vec![0xCC; 512];

    let synth_records = vec![
        ("cpio_be/file1.txt", payload1.as_slice(), 0o100644, 1_650_000_000),
        ("cpio_be/file2.bin", payload2.as_slice(), 0o100755, 1_650_000_100),
    ];

    let bytes = synthesize_binary_cpio(&synth_records, true);
    assert_eq!(&bytes[0..2], &[0x71, 0xC7], "Must match big-endian CPIO magic 0x71C7");

    let extracted = read_archive_buffer(&bytes).expect("Failed to read CPIO Binary Big-Endian");
    assert_eq!(extracted.len(), 2);
    assert_eq!(extracted[0].path, "cpio_be/file1.txt");
    assert_eq!(extracted[0].data, payload1);
    assert_eq!(extracted[0].sha256, compute_sha256(payload1));
    assert_eq!(extracted[1].path, "cpio_be/file2.bin");
    assert_eq!(extracted[1].data, payload2);
    assert_eq!(extracted[1].sha256, compute_sha256(&payload2));
}

/// 5. Binary Little-Endian CPIO (0xC771).
pub fn run_cpio_binary_le_matrix_test() {
    let payload1 = b"Binary Little Endian CPIO Payload Test";
    let payload2 = vec![0xEE; 1024];

    let synth_records = vec![
        ("cpio_le/alpha.txt", payload1.as_slice(), 0o100644, 1_670_000_000),
        ("cpio_le/beta.dat", payload2.as_slice(), 0o100644, 1_670_000_200),
    ];

    let bytes = synthesize_binary_cpio(&synth_records, false);
    assert_eq!(&bytes[0..2], &[0xC7, 0x71], "Must match little-endian CPIO magic bytes 0xC7, 0x71");

    let extracted = read_archive_buffer(&bytes).expect("Failed to read CPIO Binary Little-Endian");
    assert_eq!(extracted.len(), 2);
    assert_eq!(extracted[0].path, "cpio_le/alpha.txt");
    assert_eq!(extracted[0].data, payload1);
    assert_eq!(extracted[0].sha256, compute_sha256(payload1));
    assert_eq!(extracted[1].path, "cpio_le/beta.dat");
    assert_eq!(extracted[1].data, payload2);
    assert_eq!(extracted[1].sha256, compute_sha256(&payload2));
}
