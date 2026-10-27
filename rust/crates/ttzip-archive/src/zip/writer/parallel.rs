// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Multithreaded parallel compression engine for ZIP archives.

use super::data_descriptor::compute_zipcrypto_check_byte;
use super::types::{unix_to_dos_time, ZipCompressedItem, ZipInputItem};
use crate::codecs::deflate::{deflate_compress, deflate_compress_bound};
use crate::crypto::crc32::crc32_fast;
use crate::crypto::sha1::winzip_aes256_encrypt_and_tag;
use crate::types::{TTZipEncryptionMethod, TTZipStatus};
use rayon::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use ttzip_types::scheduler::LocalWorkStealingPool;

/// Compresses a single input item in-place with zero unnecessary buffer cloning.
#[inline]
fn compress_single_item(
    item: ZipInputItem,
    level: i32,
    encryption: TTZipEncryptionMethod,
    password: Option<&str>,
) -> Result<ZipCompressedItem, TTZipStatus> {
    if item.is_directory || item.data.is_empty() {
        return Ok(ZipCompressedItem {
            rel_path: item.rel_path,
            uncompressed_size: 0,
            compressed_size: 0,
            crc32: 0,
            compression_method: 0,
            actual_method: 0,
            aes_strength: 0,
            payload: Vec::new(),
            mtime_epoch_secs: item.mtime_epoch_secs,
            mode: item.mode,
            is_directory: item.is_directory,
            is_encrypted: false,
        });
    }

    let uncompressed_size = item.data.len() as u64;
    let crc32 = crc32_fast(0, &item.data);

    let (actual_method, raw_payload) = if level == 0 {
        (0u16, item.data)
    } else {
        let mut comp_buf = vec![0u8; deflate_compress_bound(item.data.len(), level)];
        let comp_len = deflate_compress(&item.data, &mut comp_buf, level)?;
        comp_buf.truncate(comp_len);
        (8u16, comp_buf)
    };

    let (compression_method, aes_strength, is_encrypted, final_payload) =
        if encryption == TTZipEncryptionMethod::Aes256 {
            let pass = password.ok_or(TTZipStatus::ErrInvalidPassword)?;
            let mut salt = [0u8; 16];
            unsafe {
                libc::arc4random_buf(salt.as_mut_ptr() as *mut libc::c_void, salt.len());
            }
            let mut enc_payload = Vec::new();
            winzip_aes256_encrypt_and_tag(pass, &salt, &raw_payload, &mut enc_payload)?;
            (99u16, 3u8, true, enc_payload)
        } else if encryption == TTZipEncryptionMethod::ZipCrypto {
            let pass = password.ok_or(TTZipStatus::ErrInvalidPassword)?;
            let mut enc_payload = Vec::with_capacity(12 + raw_payload.len());
            let mut header = [0u8; 12];
            unsafe {
                libc::arc4random_buf(header.as_mut_ptr() as *mut libc::c_void, 11);
            }
            let (dos_time, _) = unix_to_dos_time(item.mtime_epoch_secs);
            header[11] = compute_zipcrypto_check_byte(crc32, dos_time, false);
            let mut keys = crate::crypto::zipcrypto::ZipCryptoKeys::from_password(pass.as_bytes());
            keys.encrypt_slice(&mut header);
            enc_payload.extend_from_slice(&header);
            let mut body = raw_payload;
            keys.encrypt_slice(&mut body);
            enc_payload.extend_from_slice(&body);
            (actual_method, 0u8, true, enc_payload)
        } else {
            (actual_method, 0u8, false, raw_payload)
        };

    let compressed_size = final_payload.len() as u64;

    Ok(ZipCompressedItem {
        rel_path: item.rel_path,
        uncompressed_size,
        compressed_size,
        crc32,
        compression_method,
        actual_method,
        aes_strength,
        payload: final_payload,
        mtime_epoch_secs: item.mtime_epoch_secs,
        mode: item.mode,
        is_directory: false,
        is_encrypted,
    })
}

/// Compresses a batch of `ZipInputItem`s in parallel across threads with cooperative cancellation support.
pub fn compress_items_parallel_with_token(
    items: Vec<ZipInputItem>,
    level: i32,
    encryption: TTZipEncryptionMethod,
    password: Option<&str>,
    thread_budget: u32,
    cancel_token: Option<&AtomicBool>,
) -> Result<Vec<ZipCompressedItem>, TTZipStatus> {
    if items.is_empty() {
        return Ok(Vec::new());
    }

    if let Some(token) = cancel_token {
        if token.load(Ordering::Acquire) {
            return Err(TTZipStatus::Cancelled);
        }
    }

    let thread_count = (thread_budget as usize).clamp(1, 64).min(items.len().max(1));
    let pool = LocalWorkStealingPool::with_threads(thread_count);

    pool.install(|| {
        items
            .into_par_iter()
            .map(|item| {
                if let Some(token) = cancel_token {
                    if token.load(Ordering::Acquire) {
                        return Err(TTZipStatus::Cancelled);
                    }
                }
                compress_single_item(item, level, encryption, password)
            })
            .collect()
    })
}

/// Compresses a batch of `ZipInputItem`s in parallel across threads using work-stealing pool.
#[inline]
pub fn compress_items_parallel(
    items: Vec<ZipInputItem>,
    level: i32,
    encryption: TTZipEncryptionMethod,
    password: Option<&str>,
    thread_budget: u32,
) -> Result<Vec<ZipCompressedItem>, TTZipStatus> {
    compress_items_parallel_with_token(items, level, encryption, password, thread_budget, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_compression_basic_roundtrip() {
        let items = vec![
            ZipInputItem {
                rel_path: "file1.txt".to_string(),
                data: b"hello world from parallel stream 1".to_vec(),
                mtime_epoch_secs: 1700000000,
                mode: 0o644,
                is_directory: false,
            },
            ZipInputItem {
                rel_path: "file2.txt".to_string(),
                data: b"another chunk of data for parallel stream 2".to_vec(),
                mtime_epoch_secs: 1700000001,
                mode: 0o644,
                is_directory: false,
            },
        ];

        let results = compress_items_parallel(items, 6, TTZipEncryptionMethod::None, None, 4)
            .expect("compression should succeed");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].rel_path, "file1.txt");
        assert_eq!(results[1].rel_path, "file2.txt");
    }

    #[test]
    fn test_parallel_compression_cooperative_cancellation() {
        let items = vec![
            ZipInputItem {
                rel_path: "file1.txt".to_string(),
                data: vec![0x42; 1024 * 64],
                mtime_epoch_secs: 1700000000,
                mode: 0o644,
                is_directory: false,
            },
        ];

        let cancel_token = AtomicBool::new(true); // Pre-cancelled
        let res = compress_items_parallel_with_token(
            items,
            6,
            TTZipEncryptionMethod::None,
            None,
            4,
            Some(&cancel_token),
        );
        assert_eq!(res, Err(TTZipStatus::Cancelled));
    }
}
