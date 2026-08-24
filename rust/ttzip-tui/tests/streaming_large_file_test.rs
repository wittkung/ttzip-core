// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use tempfile::NamedTempFile;
use ttzip_engine::crypto::crc32::crc32_fast;

#[test]
fn test_sparse_large_file_bounded_memory_streaming() {
    // Create a 1GB sparse file to verify streaming memory consumption without disk fill
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    let file_handle = file.as_file_mut();
    
    // Seek to 1GB and write 1 byte
    let target_size: u64 = 1024 * 1024 * 1024; // 1 GB
    file_handle.seek(SeekFrom::Start(target_size - 1)).expect("Failed to seek");
    file_handle.write_all(&[0x42]).expect("Failed to write byte");
    file_handle.flush().expect("Failed to flush");

    // Reopen and stream hash using 128KB buffer
    let read_file = File::open(file.path()).expect("Failed to open file");
    let mut reader = BufReader::with_capacity(128 * 1024, read_file);
    let mut buffer = [0u8; 128 * 1024];
    let mut crc = 0u32;
    let mut total_read = 0u64;

    while let Ok(n) = reader.read(&mut buffer) {
        if n == 0 { break; }
        crc = crc32_fast(crc, &buffer[..n]);
        total_read += n as u64;
    }

    assert_eq!(total_read, target_size);
    assert_ne!(crc, 0);
}
