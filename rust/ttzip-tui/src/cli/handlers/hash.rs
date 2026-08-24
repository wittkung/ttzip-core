// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Subcommand execution handler: hash / checksum.

use crate::cli::args::HashResultDto;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::Instant;
use ttzip_engine::crypto::adler32::adler32_fast;
use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::crypto::crc64::crc64;
use ttzip_engine::crypto::sha256::FastSha256;

const CHUNK_SIZE: usize = 128 * 1024; // 128 KB chunk buffer

/// Executes headless `hash` subcommand with streaming chunked reader.
pub fn execute_hash(path: &Path, algorithm: &str, json: bool) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("File or archive not found: {}", path.display()));
    }

    let start = Instant::now();
    let mut file = File::open(path)
        .map_err(|e| format!("Failed to open target file {}: {}", path.display(), e))?;

    let algo_lower = algorithm.to_lowercase();
    let compute_all = algo_lower == "all";
    let compute_crc32 = compute_all || algo_lower == "crc32";
    let compute_crc64 = compute_all || algo_lower == "crc64";
    let compute_adler32 = compute_all || algo_lower == "adler32";
    let compute_sha256 = compute_all || algo_lower == "sha256";

    let mut running_crc32: u32 = 0;
    let mut running_crc64: u64 = 0;
    let mut running_adler32: u32 = 1;
    let mut sha256_hasher = FastSha256::new();

    let mut total_bytes: u64 = 0;
    let mut buffer = vec![0u8; CHUNK_SIZE];

    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|e| format!("Failed to read chunk from {}: {}", path.display(), e))?;
        if n == 0 {
            break;
        }

        let chunk = &buffer[..n];
        total_bytes += n as u64;

        if compute_crc32 {
            running_crc32 = crc32_fast(running_crc32, chunk);
        }
        if compute_crc64 {
            running_crc64 = crc64(chunk, running_crc64);
        }
        if compute_adler32 {
            running_adler32 = adler32_fast(running_adler32, chunk);
        }
        if compute_sha256 {
            sha256_hasher.update(chunk);
        }
    }

    let crc32_val = if compute_crc32 {
        Some(format!("0x{:08X}", running_crc32))
    } else {
        None
    };

    let crc64_val = if compute_crc64 {
        Some(format!("0x{:016X}", running_crc64))
    } else {
        None
    };

    let adler32_val = if compute_adler32 {
        Some(format!("0x{:08X}", running_adler32))
    } else {
        None
    };

    let sha256_val = if compute_sha256 {
        let digest = sha256_hasher.finalize();
        Some(hex_encode(&digest))
    } else {
        None
    };

    let elapsed = start.elapsed().as_millis() as u64;

    if json {
        let dto = HashResultDto {
            target: path.to_string_lossy().to_string(),
            size_bytes: total_bytes,
            crc32: crc32_val,
            crc64: crc64_val,
            sha256: sha256_val,
            adler32: adler32_val,
            elapsed_ms: elapsed,
        };

        let json_str = serde_json::to_string_pretty(&dto)
            .map_err(|e| format!("Failed to serialize hash JSON: {}", e))?;
        println!("{}", json_str);
        return Ok(());
    }

    println!("{:=<60}", "");
    println!("  TTZip Checksum & Digest: {}", path.display());
    println!("{:=<60}", "");
    println!("  File Size:  {} bytes", total_bytes);
    if let Some(ref c32) = crc32_val {
        println!("  CRC-32:     {}", c32);
    }
    if let Some(ref c64) = crc64_val {
        println!("  CRC-64:     {}", c64);
    }
    if let Some(ref adler) = adler32_val {
        println!("  Adler-32:   {}", adler);
    }
    if let Some(ref s256) = sha256_val {
        println!("  SHA-256:    {}", s256);
    }
    println!("  Elapsed:    {} ms", elapsed);
    println!("{:=<60}", "");

    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}
