// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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

struct RunningHashes {
    compute_crc32: bool,
    compute_crc64: bool,
    compute_adler32: bool,
    compute_sha256: bool,
    running_crc32: u32,
    running_crc64: u64,
    running_adler32: u32,
    sha256_hasher: FastSha256,
    total_bytes: u64,
}

impl RunningHashes {
    fn new(algorithm: &str) -> Self {
        let algo = algorithm.to_lowercase();
        let all = algo == "all";
        Self {
            compute_crc32: all || algo == "crc32",
            compute_crc64: all || algo == "crc64",
            compute_adler32: all || algo == "adler32",
            compute_sha256: all || algo == "sha256",
            running_crc32: 0,
            running_crc64: 0,
            running_adler32: 1,
            sha256_hasher: FastSha256::new(),
            total_bytes: 0,
        }
    }

    fn update(&mut self, chunk: &[u8]) {
        self.total_bytes += chunk.len() as u64;
        if self.compute_crc32 {
            self.running_crc32 = crc32_fast(self.running_crc32, chunk);
        }
        if self.compute_crc64 {
            self.running_crc64 = crc64(chunk, self.running_crc64);
        }
        if self.compute_adler32 {
            self.running_adler32 = adler32_fast(self.running_adler32, chunk);
        }
        if self.compute_sha256 {
            self.sha256_hasher.update(chunk);
        }
    }

    fn into_dto(self, path: &Path, elapsed_ms: u64) -> HashResultDto {
        HashResultDto {
            target: path.to_string_lossy().to_string(),
            size_bytes: self.total_bytes,
            crc32: self.compute_crc32.then(|| format!("0x{:08X}", self.running_crc32)),
            crc64: self.compute_crc64.then(|| format!("0x{:016X}", self.running_crc64)),
            sha256: self.compute_sha256.then(|| hex_encode(&self.sha256_hasher.finalize())),
            adler32: self.compute_adler32.then(|| format!("0x{:08X}", self.running_adler32)),
            elapsed_ms,
        }
    }
}

fn compute_file_hashes(path: &Path, algorithm: &str) -> Result<HashResultDto, String> {
    let start = Instant::now();
    let mut file = File::open(path)
        .map_err(|e| format!("Failed to open target file {}: {}", path.display(), e))?;
    let mut hashes = RunningHashes::new(algorithm);
    let mut buffer = vec![0u8; CHUNK_SIZE];

    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|e| format!("Failed to read chunk from {}: {}", path.display(), e))?;
        if n == 0 {
            break;
        }
        hashes.update(&buffer[..n]);
    }

    Ok(hashes.into_dto(path, start.elapsed().as_millis() as u64))
}

/// Executes headless `hash` subcommand with streaming chunked reader.
pub fn execute_hash(path: &Path, algorithm: &str, json: bool) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("File or archive not found: {}", path.display()));
    }

    let dto = compute_file_hashes(path, algorithm)?;
    if json {
        let json_str = serde_json::to_string_pretty(&dto)
            .map_err(|e| format!("Failed to serialize hash JSON: {}", e))?;
        println!("{}", json_str);
        return Ok(());
    }

    println!("{:=<60}", "");
    println!("  TTZip Checksum & Digest: {}", path.display());
    println!("{:=<60}", "");
    println!("  File Size:  {} bytes", dto.size_bytes);
    if let Some(ref c32) = dto.crc32 {
        println!("  CRC-32:     {}", c32);
    }
    if let Some(ref c64) = dto.crc64 {
        println!("  CRC-64:     {}", c64);
    }
    if let Some(ref adler) = dto.adler32 {
        println!("  Adler-32:   {}", adler);
    }
    if let Some(ref s256) = dto.sha256 {
        println!("  SHA-256:    {}", s256);
    }
    println!("  Elapsed:    {} ms", dto.elapsed_ms);
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
