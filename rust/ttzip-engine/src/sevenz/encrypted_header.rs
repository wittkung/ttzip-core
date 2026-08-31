// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 7-Zip Encrypted Header (`kEncodedHeader`, 0x17) Recursive Self-Extracting
//! State Machine and Sub-Millisecond Fast Password Probing Engine.
//!
//! Provides:
//! 1. `EncodedHeaderDecoder`: Robust recursive decoding of encoded & encrypted
//!    7z headers with 4MB memory exhaustion protection (`MAX_PREALLOC_BYTES`),
//!    AES-256-CBC hardware decryption, and CRC32 verification.
//! 2. `probe_7z_password`: Lightweight, sub-millisecond password probing based on
//!    three natural defense lines (LZMA2 chunk/range assertion -> Header NID
//!    assertion -> CRC32 verification).

use std::io::{Read, Seek, SeekFrom};
use zeroize::Zeroizing;

use super::dag::SevenZError;
use super::format::*;
use super::header::{parse_7z_header_stream, SevenZFolder, SevenZHeaderInfo};
use super::sanitizer::bounded_usize;
use crate::codecs::deflate::deflate_decompress;
use crate::crypto::aes256::aes256_cbc_decrypt;
use crate::crypto::crc32::crc32_fast;
use crate::crypto::sha256::sha256_7z_kdf;

/// Maximum allowable uncompressed header size (4 MiB) to guard against zip-bomb OOM.
pub const MAX_PREALLOC_BYTES: usize = 4 * 1024 * 1024;

/// Maximum allowable packed stream size for an encoded header (64 MiB).
pub const MAX_PACKED_HEADER_BYTES: usize = 64 * 1024 * 1024;

/// Valid top-level property tags (NIDs) inside an uncompressed 7z `kHeader` stream.
const VALID_TOP_LEVEL_NIDS: &[u8] = &[
    K_HEADER,
    K_MAIN_STREAMS_INFO,
    K_FILES_INFO,
    K_ARCHIVE_PROPERTIES,
    K_ADDITIONAL_STREAMS_INFO,
    K_END,
];

/// Recursive decoder for 7-Zip `kEncodedHeader` (0x17) metadata blocks.
#[derive(Debug, Clone)]
pub struct EncodedHeaderDecoder {
    max_unpack_limit: usize,
}

impl Default for EncodedHeaderDecoder {
    fn default() -> Self {
        Self::new(MAX_PREALLOC_BYTES)
    }
}

impl EncodedHeaderDecoder {
    /// Creates a new `EncodedHeaderDecoder` with a custom memory growth limit in bytes.
    #[must_use]
    pub const fn new(max_unpack_limit: usize) -> Self {
        Self { max_unpack_limit }
    }

    /// Decodes the 7z header from an in-memory mapped archive slice, transparently
    /// handling both plain headers (`0x01`) and recursive `kEncodedHeader` (`0x17`) streams.
    ///
    /// # Errors
    /// Returns `SevenZError` on corrupt headers, incorrect password, or memory limit violation.
    pub fn decode(
        &self,
        mapped: &[u8],
        password: Option<&str>,
    ) -> Result<SevenZHeaderInfo, SevenZError> {
        let sig = SevenZSignatureHeader::parse(mapped)?;
        let header_start = 32usize.saturating_add(sig.next_header_offset as usize);
        let header_size = sig.next_header_size as usize;

        if header_start.saturating_add(header_size) > mapped.len() {
            return Err(SevenZError::CorruptHeader("Header offset out of archive bounds"));
        }

        let header_bytes = &mapped[header_start..header_start + header_size];

        if sig.next_header_crc != 0 && header_size > 0 {
            let computed_crc = crc32_fast(0, header_bytes);
            if computed_crc != sig.next_header_crc {
                return Err(SevenZError::CrcMismatch {
                    expected: sig.next_header_crc,
                    computed: computed_crc,
                });
            }
        }

        if header_bytes.is_empty() {
            return Ok(SevenZHeaderInfo::default());
        }

        if header_bytes[0] == K_ENCODED_HEADER {
            self.decode_encoded_header(&header_bytes[1..], mapped, password)
        } else {
            let mut info = SevenZHeaderInfo {
                payload_offset: 32,
                payload_len: sig.next_header_offset as usize,
                ..Default::default()
            };
            parse_7z_header_stream(header_bytes, &mut info)?;
            Ok(info)
        }
    }

    /// Decodes a raw `kEncodedHeader` boot stream (`header_bytes[1..]`) by decompressing
    /// and decrypting the inner header folder, then parsing the resulting `kHeader` stream.
    pub fn decode_encoded_header(
        &self,
        boot_stream: &[u8],
        mapped: &[u8],
        password: Option<&str>,
    ) -> Result<SevenZHeaderInfo, SevenZError> {
        let mut boot_info = SevenZHeaderInfo::default();
        parse_7z_header_stream(boot_stream, &mut boot_info)?;

        if boot_info.folders.is_empty() {
            return Err(SevenZError::CorruptHeader("Missing folder in encoded header"));
        }

        let folder = &boot_info.folders[0];
        let unpacked_bytes = decompress_header_folder(
            folder,
            &boot_info,
            mapped,
            password,
            self.max_unpack_limit,
        )?;

        let mut final_info = SevenZHeaderInfo::default();
        if !unpacked_bytes.is_empty() {
            if unpacked_bytes[0] == K_HEADER {
                parse_7z_header_stream(&unpacked_bytes[1..], &mut final_info)?;
            } else {
                parse_7z_header_stream(&unpacked_bytes, &mut final_info)?;
            }
        }

        Ok(final_info)
    }
}

/// Decompresses and decrypts a specific 7z header folder with strict memory allocation bounds.
fn decompress_header_folder(
    folder: &SevenZFolder,
    info: &SevenZHeaderInfo,
    mapped: &[u8],
    password: Option<&str>,
    max_unpack_limit: usize,
) -> Result<Vec<u8>, SevenZError> {
    let packed_offset = folder.packed_offset;
    let packed_len = folder.packed_len;

    if packed_len > MAX_PACKED_HEADER_BYTES {
        return Err(SevenZError::CountLimitExceeded {
            field_name: "packed header size",
            value: packed_len as u64,
            limit: MAX_PACKED_HEADER_BYTES,
        });
    }

    if packed_offset.saturating_add(packed_len) > mapped.len() {
        return Err(SevenZError::CorruptHeader("Packed header stream out of bounds"));
    }

    let raw_payload = &mapped[packed_offset..packed_offset + packed_len];
    let is_aes_encrypted = info.is_encrypted || folder.coders.iter().any(|c| c.method_id == METHOD_AES);

    let decrypted_storage = if is_aes_encrypted {
        let pass = password.ok_or(SevenZError::BadPassword)?;
        if pass.is_empty() {
            return Err(SevenZError::BadPassword);
        }

        if !raw_payload.len().is_multiple_of(16) {
            return Err(SevenZError::CorruptHeader("AES payload length not multiple of 16"));
        }

        let key = Zeroizing::new(sha256_7z_kdf(
            pass,
            &info.aes_salt[..info.aes_salt_len],
            info.aes_num_cycles_power,
        ));

        let mut dec = Zeroizing::new(vec![0u8; raw_payload.len()]);
        aes256_cbc_decrypt(&key, &info.aes_iv, raw_payload, &mut dec)
            .map_err(|_| SevenZError::BadPassword)?;
        dec
    } else {
        Zeroizing::new(raw_payload.to_vec())
    };

    let (method_id, coder_props) = folder
        .coders
        .iter()
        .find(|c| c.method_id != METHOD_AES)
        .map(|c| (c.method_id, c.properties.as_slice()))
        .unwrap_or((info.primary_method_id, info.coder_props.as_slice()));

    let raw_unpack_sz = folder
        .unpack_sizes
        .first()
        .copied()
        .or_else(|| folder.unpack_sizes.last().copied())
        .unwrap_or(0);
    let expected_unpack_size = bounded_usize(raw_unpack_sz, max_unpack_limit, "header unpack size")?;

    let unpacked = decompress_header_bytes(
        &decrypted_storage,
        method_id,
        coder_props,
        expected_unpack_size,
        max_unpack_limit,
    ).map_err(|err| {
        if is_aes_encrypted {
            SevenZError::BadPassword
        } else {
            err
        }
    })?;

    if let Some(expected_crc) = folder.crc {
        let computed_crc = crc32_fast(0, &unpacked);
        if computed_crc != expected_crc {
            if is_aes_encrypted {
                return Err(SevenZError::BadPassword);
            }
            return Err(SevenZError::CrcMismatch {
                expected: expected_crc,
                computed: computed_crc,
            });
        }
    }

    Ok(unpacked)
}

/// Decompresses raw in-memory header bytes according to the specified 7z compression method ID.
fn decompress_header_bytes(
    input: &[u8],
    method_id: u64,
    coder_props: &[u8],
    expected_unpack_size: usize,
    max_limit: usize,
) -> Result<Vec<u8>, SevenZError> {
    match method_id {
        METHOD_COPY => {
            let mut out = input.to_vec();
            if expected_unpack_size > 0 && expected_unpack_size < out.len() {
                out.truncate(expected_unpack_size);
            }
            if out.len() > max_limit {
                return Err(SevenZError::CountLimitExceeded {
                    field_name: "copy header size",
                    value: out.len() as u64,
                    limit: max_limit,
                });
            }
            Ok(out)
        }
        METHOD_LZMA2 => {
            let mut dstream = crate::codecs::lzma2::Fl2DStream::new(1)
                .map_err(|e| SevenZError::DecompressionFailed(format!("LZMA2 init: {e:?}")))?;
            let init_res = dstream.init(coder_props.first().copied());
            if init_res.is_err() {
                dstream.init(None)
                    .map_err(|e| SevenZError::DecompressionFailed(format!("LZMA2 prop init fallback: {e:?}")))?;
            }

            let mut in_buf = crate::codecs::lzma2::Fl2InBuffer {
                src: input.as_ptr() as *const libc::c_void,
                size: input.len(),
                pos: 0,
            };

            let alloc_sz = if expected_unpack_size > 0 {
                expected_unpack_size
            } else {
                max_limit.min(64 * 1024)
            };
            let mut out = vec![0u8; alloc_sz];
            let mut total_out = 0usize;

            while in_buf.pos < in_buf.size {
                let prev_in = in_buf.pos;
                if total_out >= out.len() {
                    if out.len() >= max_limit {
                        return Err(SevenZError::CountLimitExceeded {
                            field_name: "header unpack size",
                            value: total_out as u64,
                            limit: max_limit,
                        });
                    }
                    out.resize(out.len().saturating_mul(2).min(max_limit), 0);
                }

                let mut out_buf = crate::codecs::lzma2::Fl2OutBuffer {
                    dst: unsafe { out.as_mut_ptr().add(total_out) as *mut libc::c_void },
                    size: out.len() - total_out,
                    pos: 0,
                };

                let remaining = dstream.decompress_stream(&mut in_buf, &mut out_buf)
                    .map_err(|e| SevenZError::DecompressionFailed(format!("LZMA2 stream error: {e:?}")))?;
                total_out += out_buf.pos;

                if remaining == 0 {
                    break;
                }
                if out_buf.pos == 0 && in_buf.pos == prev_in {
                    return Err(SevenZError::DecompressionFailed("LZMA2 stall without progress".to_string()));
                }
            }

            out.truncate(total_out);
            Ok(out)
        }
        METHOD_LZMA => {
            if coder_props.len() < 5 {
                return Err(SevenZError::CorruptHeader("LZMA properties too short"));
            }
            let mut out = vec![0u8; expected_unpack_size];
            let decomp_len = crate::codecs::lzma::lzma1_decompress(
                input,
                coder_props,
                expected_unpack_size as u64,
                &mut out,
            )
            .map_err(|e| SevenZError::DecompressionFailed(format!("LZMA error: {e:?}")))?;
            out.truncate(decomp_len);
            Ok(out)
        }
        METHOD_DEFLATE => {
            let mut out = vec![0u8; expected_unpack_size];
            let decomp_len = deflate_decompress(input, &mut out)
                .map_err(|e| SevenZError::DecompressionFailed(format!("Deflate error: {e:?}")))?;
            out.truncate(decomp_len);
            Ok(out)
        }
        METHOD_BZIP2 => {
            let mut out = vec![0u8; expected_unpack_size];
            let decomp_len = crate::codecs::bzip2::bzip2_decompress(input, &mut out)
                .map_err(|e| SevenZError::DecompressionFailed(format!("BZip2 error: {e:?}")))?;
            out.truncate(decomp_len);
            Ok(out)
        }
        METHOD_PPMD => {
            let mut out = vec![0u8; expected_unpack_size];
            let decomp_len = crate::codecs::ppmd::ppmd_decompress_7z(input, &mut out, coder_props)
                .map_err(|e| SevenZError::DecompressionFailed(format!("PPMd error: {e:?}")))?;
            out.truncate(decomp_len);
            Ok(out)
        }
        other => Err(SevenZError::UnsupportedCodec(other)),
    }
}

/// Lightweight, sub-millisecond 7-Zip password probe (`probe_7z_password`).
///
/// Employs three natural defense lines to ascertain password validity in $< 1\text{ms}$:
/// 1. **Defense Line 1 (Codec Header / Range Assertion)**: Asserts LZMA2 chunk control byte
///    legality (`0x00`, `0x01`, `0x02`, `0x80..=0xFF`, rejecting `0x03..=0x7F`) and LZMA property bounds (`< 225`).
/// 2. **Defense Line 2 (Header NID Assertion)**: Verifies the presence of valid 7z header property tags
///    (`kHeader 0x01`, `kMainStreamsInfo 0x04`, `kFilesInfo 0x05`, etc.) in the uncompressed stream.
/// 3. **Defense Line 3 (CRC32 Verification)**: Validates unpacked folder and header CRC32 checksums.
///
/// # Returns
/// - `Ok(true)` if the password is confirmed valid or archive is unencrypted.
/// - `Err(SevenZError::BadPassword)` or `Err(SevenZError::MaybeBadPassword)` if password is incorrect.
pub fn probe_7z_password<R: Read + Seek>(
    reader: &mut R,
    password: &str,
) -> Result<bool, SevenZError> {
    reader.seek(SeekFrom::Start(0))?;
    let mut sig_buf = [0u8; 32];
    reader.read_exact(&mut sig_buf).map_err(|e| SevenZError::Io(e.to_string()))?;

    let sig = SevenZSignatureHeader::parse(&sig_buf)?;
    if sig.next_header_size == 0 {
        return Ok(true);
    }

    let header_size = bounded_usize(sig.next_header_size, MAX_PREALLOC_BYTES, "probe next header size")?;
    reader.seek(SeekFrom::Start(32 + sig.next_header_offset)).map_err(|e| SevenZError::Io(e.to_string()))?;

    let mut next_header_bytes = vec![0u8; header_size];
    reader.read_exact(&mut next_header_bytes).map_err(|e| SevenZError::Io(e.to_string()))?;

    if sig.next_header_crc != 0 {
        let computed = crc32_fast(0, &next_header_bytes);
        if computed != sig.next_header_crc {
            return Err(SevenZError::CrcMismatch {
                expected: sig.next_header_crc,
                computed,
            });
        }
    }

    let first_byte = next_header_bytes[0];
    if first_byte == K_HEADER {
        // Plaintext header: no password required for header
        return Ok(true);
    }

    if first_byte != K_ENCODED_HEADER {
        return Err(SevenZError::CorruptHeader("Unexpected next header identifier"));
    }

    let mut boot_info = SevenZHeaderInfo::default();
    parse_7z_header_stream(&next_header_bytes[1..], &mut boot_info)?;

    let is_aes_encrypted = boot_info.is_encrypted
        || boot_info.folders.iter().any(|f| f.coders.iter().any(|c| c.method_id == METHOD_AES));

    if !is_aes_encrypted {
        return Ok(true);
    }

    if password.is_empty() {
        return Err(SevenZError::BadPassword);
    }

    let folder = boot_info
        .folders
        .first()
        .ok_or(SevenZError::CorruptHeader("Missing folder in encoded header"))?;

    let packed_len = folder.packed_len;
    if packed_len == 0 {
        return Ok(true);
    }

    if !packed_len.is_multiple_of(16) {
        return Err(SevenZError::CorruptHeader("AES packed length not multiple of 16"));
    }

    reader.seek(SeekFrom::Start(folder.packed_offset as u64)).map_err(|e| SevenZError::Io(e.to_string()))?;

    let mut packed_bytes = vec![0u8; packed_len];
    reader.read_exact(&mut packed_bytes).map_err(|e| SevenZError::Io(e.to_string()))?;

    let key = Zeroizing::new(sha256_7z_kdf(
        password,
        &boot_info.aes_salt[..boot_info.aes_salt_len],
        boot_info.aes_num_cycles_power,
    ));

    let mut decrypted = Zeroizing::new(vec![0u8; packed_bytes.len()]);
    aes256_cbc_decrypt(&key, &boot_info.aes_iv, &packed_bytes, &mut decrypted)
        .map_err(|_| SevenZError::BadPassword)?;

    // ========================================================================
    // Defense Line 1: Codec Header / Range Assertion
    // ========================================================================
    let primary_coder = folder
        .coders
        .iter()
        .find(|c| c.method_id != METHOD_AES)
        .or_else(|| folder.coders.first());

    let method_id = primary_coder.map(|c| c.method_id).unwrap_or(boot_info.primary_method_id);

    if method_id == METHOD_LZMA2 && !decrypted.is_empty() {
        let ctrl = decrypted[0];
        // 0x03..=0x7F are strictly invalid control bytes in LZMA2 specification
        if (0x03..=0x7F).contains(&ctrl) {
            return Err(SevenZError::BadPassword);
        }
        if ctrl >= 0x80 && (ctrl & 0x40) != 0 && decrypted.len() >= 2 {
            let props = decrypted[1];
            if props >= 225 {
                return Err(SevenZError::BadPassword);
            }
        }
    } else if method_id == METHOD_LZMA {
        if let Some(coder) = primary_coder {
            if coder.properties.len() >= 5 && coder.properties[0] >= 225 {
                return Err(SevenZError::BadPassword);
            }
        }
    }

    // ========================================================================
    // Defense Line 2: Decompression & Header NID Assertion
    // ========================================================================
    let expected_unpack = folder
        .unpack_sizes
        .first()
        .copied()
        .or_else(|| folder.unpack_sizes.last().copied())
        .unwrap_or(0);
    let unpack_limit = bounded_usize(expected_unpack, MAX_PREALLOC_BYTES, "probe header unpack size")?;

    let coder_props = primary_coder.map(|c| c.properties.as_slice()).unwrap_or(&boot_info.coder_props);
    let decomp = match decompress_header_bytes(&decrypted, method_id, coder_props, unpack_limit, MAX_PREALLOC_BYTES) {
        Ok(d) => d,
        Err(_) => return Err(SevenZError::BadPassword),
    };

    if decomp.is_empty() {
        if expected_unpack > 0 {
            return Err(SevenZError::BadPassword);
        }
        return Ok(true);
    }

    let first_tag = decomp[0];
    if !VALID_TOP_LEVEL_NIDS.contains(&first_tag) {
        return Err(SevenZError::BadPassword);
    }

    // ========================================================================
    // Defense Line 3: CRC32 Checksum Validation
    // ========================================================================
    if let Some(expected_crc) = folder.crc {
        let computed = crc32_fast(0, &decomp);
        if computed != expected_crc {
            return Err(SevenZError::BadPassword);
        }
    }

    Ok(true)
}
