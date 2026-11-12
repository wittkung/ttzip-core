// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Subcommand execution handler: recover (multi-core dictionary attack with real-time speed).

use crate::cli::args::RecoverResultDto;
use crate::cli::format::{detect_archive_format, read_archive_data_auto, ContainerFormat};
use std::fs;
use std::path::Path;
use std::time::Instant;
use ttzip_engine::crypto::recovery::{
    recover_7z_aes_rayon, recover_dictionary_rayon, recover_zipcrypto_rayon, RecoveryTarget,
};
use ttzip_engine::sevenz::SevenZArchive;
use ttzip_engine::zip::parser::parse_local_file_header;
use ttzip_engine::zip::ZipArchive;

/// Executes headless `recover` subcommand.
pub fn execute_recover(
    archive_path: &Path,
    dictionary_path: &Path,
    threads_opt: Option<u32>,
    json: bool,
) -> Result<(), String> {
    if !archive_path.exists() {
        return Err(format!("Archive file not found: {}", archive_path.display()));
    }
    if !dictionary_path.exists() {
        return Err(format!(
            "Dictionary file not found: {}",
            dictionary_path.display()
        ));
    }

    let dict_text = fs::read_to_string(dictionary_path)
        .map_err(|e| format!("Failed to read dictionary file: {}", e))?;
    let words: Vec<&str> = dict_text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    if words.is_empty() {
        return Err("Dictionary file contains no valid words".to_string());
    }

    let (_volumes, data) = read_archive_data_auto(archive_path)?;
    let format = detect_archive_format(archive_path, &data);

    let thread_count = threads_opt.unwrap_or_else(|| rayon::current_num_threads().max(1) as u32);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(thread_count as usize)
        .build()
        .map_err(|e| format!("Failed to build Rayon thread pool: {}", e))?;

    let start_time = Instant::now();
    let total_words = words.len();
    let (recovered_password, tested_count) = match format {
        ContainerFormat::Zip => recover_zip_password(&data, &words, &pool, json, start_time)?,
        ContainerFormat::SevenZip => {
            recover_sevenz_password(&data, &words, &pool, json, start_time)?
        }
        _ => {
            return Err(format!(
                "Password recovery not supported for format: {}",
                format.name()
            ));
        }
    };

    print_recover_result(
        archive_path,
        recovered_password,
        tested_count,
        total_words,
        start_time.elapsed(),
        json,
    )
}

fn run_chunked_recovery_loop<F>(
    words: &[&str],
    chunk_size: usize,
    label: &str,
    json: bool,
    start_time: Instant,
    mut test_chunk: F,
) -> (Option<String>, usize)
where
    F: FnMut(&[&str]) -> Option<String>,
{
    let total_words = words.len();
    let mut tested_count = 0usize;
    let mut recovered_password = None;

    for chunk in words.chunks(chunk_size) {
        let found = test_chunk(chunk);
        tested_count += chunk.len();

        if !json {
            let elapsed_secs = start_time.elapsed().as_secs_f64().max(0.0001);
            let speed = tested_count as f64 / elapsed_secs;
            let pct = (tested_count as f64 / total_words as f64) * 100.0;
            print!(
                "\r[Recovering {}] {:.1}% ({}/{} keys) - Speed: {:.0} keys/s",
                label, pct, tested_count, total_words, speed
            );
            let _ = std::io::Write::flush(&mut std::io::stdout());
        }

        if let Some(pwd) = found {
            recovered_password = Some(pwd);
            break;
        }
    }

    (recovered_password, tested_count)
}

fn locate_encrypted_zip_payload<'a>(
    archive: &'a ZipArchive<'a>,
    data: &'a [u8],
) -> Result<(&'a ttzip_engine::zip::ZipEntry, &'a [u8]), String> {
    let enc_entry = archive
        .entries()
        .iter()
        .find(|e| e.is_encrypted)
        .ok_or_else(|| "No encrypted entries found in ZIP archive".to_string())?;

    let (payload_offset, _) = parse_local_file_header(data, enc_entry.lfh_offset as usize)
        .map_err(|e| format!("Failed to parse local file header: {:?}", e))?;
    let comp_size = enc_entry.compressed_size as usize;
    let payload = data
        .get(payload_offset..payload_offset + comp_size)
        .ok_or_else(|| "Corrupted ZIP payload boundary".to_string())?;
    Ok((enc_entry, payload))
}

fn recover_zip_password(
    data: &[u8],
    words: &[&str],
    pool: &rayon::ThreadPool,
    json: bool,
    start_time: Instant,
) -> Result<(Option<String>, usize), String> {
    let archive =
        ZipArchive::open_slice(data).map_err(|e| format!("Failed to open ZIP archive: {:?}", e))?;
    let (enc_entry, raw_payload) = locate_encrypted_zip_payload(&archive, data)?;

    if enc_entry.aes_strength > 0 || enc_entry.compression_method == 99 {
        let (salt_len, label) = match enc_entry.aes_strength {
            1 => (8, "WinZip AES-128"),
            2 => (12, "WinZip AES-192"),
            _ => (16, "WinZip AES-256"),
        };
        if raw_payload.len() < salt_len + 2 {
            return Err("Insufficient WinZip AES payload length".to_string());
        }
        let target = RecoveryTarget::WinZipAes {
            salt: raw_payload[0..salt_len].to_vec(),
            stored_pvv: [raw_payload[salt_len], raw_payload[salt_len + 1]],
        };
        Ok(run_chunked_recovery_loop(
            words,
            5000,
            label,
            json,
            start_time,
            |chunk| pool.install(|| recover_dictionary_rayon(chunk, &target, None, None)),
        ))
    } else {
        let enc_header: [u8; 12] = raw_payload
            .get(0..12)
            .and_then(|s| s.try_into().ok())
            .ok_or_else(|| "Insufficient ZipCrypto payload length".to_string())?;
        let check_byte = (enc_entry.crc32 >> 24) as u8;
        Ok(run_chunked_recovery_loop(
            words,
            5000,
            "ZipCrypto",
            json,
            start_time,
            |chunk| pool.install(|| recover_zipcrypto_rayon(chunk, &enc_header, check_byte)),
        ))
    }
}

fn recover_sevenz_password(
    data: &[u8],
    words: &[&str],
    pool: &rayon::ThreadPool,
    json: bool,
    start_time: Instant,
) -> Result<(Option<String>, usize), String> {
    let archive = SevenZArchive::open_slice(data)
        .map_err(|e| format!("Failed to open 7z archive: {:?}", e))?;
    let info = archive.info();
    if !info.is_encrypted {
        return Err("7-Zip archive is not password encrypted".to_string());
    }

    let salt = &info.aes_salt[..info.aes_salt_len];
    let num_cycles_power = info.aes_num_cycles_power;
    let probe_slice = if info.payload_len > 0 && info.payload_offset + info.payload_len <= data.len()
    {
        &data[info.payload_offset..info.payload_offset + info.payload_len.min(64)]
    } else {
        &[]
    };

    Ok(run_chunked_recovery_loop(
        words,
        1000,
        "7z AES-256",
        json,
        start_time,
        |chunk| {
            pool.install(|| {
                recover_7z_aes_rayon(chunk, salt, num_cycles_power, probe_slice, &[])
            })
        },
    ))
}

fn print_recover_result(
    archive_path: &Path,
    recovered_password: Option<String>,
    tested_count: usize,
    total_words: usize,
    elapsed: std::time::Duration,
    json: bool,
) -> Result<(), String> {
    let elapsed_ms = elapsed.as_millis() as u64;
    let elapsed_secs = elapsed.as_secs_f64().max(0.0001);
    let speed_keys_per_sec = tested_count as f64 / elapsed_secs;

    if json {
        let dto = RecoverResultDto {
            archive: archive_path.to_string_lossy().to_string(),
            recovered: recovered_password.is_some(),
            password: recovered_password,
            total_tested: tested_count,
            elapsed_ms,
            speed_keys_per_sec,
        };
        let json_str = serde_json::to_string_pretty(&dto)
            .map_err(|e| format!("Failed to serialize recovery JSON: {}", e))?;
        println!("{}", json_str);
        return Ok(());
    }

    println!(); // Clear progress line
    println!("{:=<80}", "");
    if let Some(ref pwd) = recovered_password {
        println!("TTZip Password Recovery: SUCCESS");
        println!("{:=<80}", "");
        println!("Archive:        {}", archive_path.display());
        println!("Password Found: \"{}\"", pwd);
    } else {
        println!("TTZip Password Recovery: FAILED");
        println!("{:=<80}", "");
        println!("Archive:        {}", archive_path.display());
        println!("Status:         Password not found in dictionary");
    }
    println!("Keys Tested:    {} / {}", tested_count, total_words);
    println!("Speed:          {:.0} keys/s", speed_keys_per_sec);
    println!("Duration:       {:.2?}", elapsed);
    println!("{:=<80}", "");

    Ok(())
}
