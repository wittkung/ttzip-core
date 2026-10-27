// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Selection of the encrypted unit that may carry a password proof.
//!
//! A confirmation target must satisfy two independent requirements:
//!
//! 1. it must carry **actual encryption evidence** — a ZIP entry whose general
//!    purpose bit 0 is set, or a 7z entry whose own folder carries an AES coder;
//!    an unencrypted unit decrypts "successfully" for any candidate and therefore
//!    carries no password information at all;
//! 2. it must carry an **integrity credential** that a candidate can fail — the
//!    CRC32 of the entry payload, or the WinZip AES authentication code.
//!
//! The selection therefore proves one single encrypted unit. It never extrapolates
//! to the other entries of a multi-password archive.

use crate::recovery::verdict::PreparationError;
use crate::sevenz::decoder::SevenZArchive;
use crate::sevenz::header::models::SevenZHeaderInfo;
use crate::sevenz::header::SevenZEntryLocation;
use crate::sevenz::format::METHOD_AES;
use crate::zip::parser::{parse_local_file_header, ZipEntry};
use crate::zip::reader::ZipArchive;

/// Minimum size of a WinZip AES payload: salt (16) + password verification (2) +
/// authentication code (10).
const WINZIP_AES_MIN_PAYLOAD: usize = 28;

/// Minimum size of a traditional ZipCrypto payload: the 12-byte encryption header.
const ZIPCRYPTO_MIN_PAYLOAD: usize = 12;

/// Which encrypted unit of the archive carries the password proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationTarget {
    /// A ZIP entry whose general purpose bit 0 marks it as encrypted.
    ZipEntry {
        /// Index of the entry inside the archive entry table.
        index: usize,
    },
    /// A 7z entry whose own folder carries an AES coder.
    SevenZipEntry {
        /// Index of the entry inside the archive entry table.
        index: usize,
        /// Index of the folder that holds the entry's packed stream.
        folder_index: usize,
    },
}

/// The integrity credential that proves a candidate for the selected unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrityCredential {
    /// CRC32 of the decompressed plaintext, compared with the CRC the archive
    /// stores for that entry.
    ///
    /// For 7z this is the CRC of the entry's own unpacked stream — exactly the value
    /// the engine compares against the extracted bytes. A folder CRC is never
    /// substituted for it, because a folder CRC covers bytes that an early
    /// terminated extraction never produced.
    EntryCrc32 {
        /// The CRC32 the archive publishes for the selected entry.
        expected: u32,
    },
    /// The WinZip AES authentication code (HMAC-SHA1-80) over the ciphertext.
    ///
    /// AE-2 stores no CRC, so for both AE versions the authentication code is the
    /// credential the engine verifies. A missing CRC is never a reason to reject an
    /// AE-2 entry.
    WinZipAesAuthentication,
}

/// The cheap-check material of the selected encrypted unit.
///
/// 7z has no entry in this vocabulary on purpose: the SHA-256 key derivation
/// dominates the cost of a 7z attempt, and no fixed magic can stand in for a
/// content-level check, so a 7z candidate has to be confirmed rather than screened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScreeningMaterial {
    /// Traditional PKWARE ZipCrypto: the 12-byte encryption header and the expected
    /// check byte.
    ZipCrypto {
        /// The encrypted 12-byte header of the selected entry.
        header: [u8; 12],
        /// The byte the decrypted header must end with.
        check_byte: u8,
    },
    /// WinZip AES: the salt and the stored password verification value.
    WinZipAes {
        /// The per-entry key derivation salt.
        salt: Vec<u8>,
        /// The stored two-byte password verification value.
        stored_pvv: [u8; 2],
    },
}

/// The encrypted unit selected for confirmation together with its credential.
pub(crate) struct SelectedUnit {
    /// Which encrypted unit was selected.
    pub target: ConfirmationTarget,
    /// The integrity credential that a candidate has to satisfy.
    pub credential: IntegrityCredential,
    /// The cheap-check material, when the format provides one.
    pub screening: Option<ScreeningMaterial>,
}

/// Selects the ZIP entry that may act as the confirmation target.
///
/// Only entries that are encrypted *and* carry a usable credential qualify. An
/// encrypted entry without payload is skipped: the archive reader accepts it for
/// every password, so it carries no evidence.
pub(crate) fn select_zip_unit(
    zip: &ZipArchive,
    data: &[u8],
) -> Result<SelectedUnit, PreparationError> {
    let mut saw_decodable = false;
    let mut saw_unsupported = false;
    for (index, entry) in zip.entries().iter().enumerate() {
        if entry.is_directory || !entry.is_encrypted {
            continue;
        }
        let payload = zip_payload(data, entry)?;
        match check_zip_entry_support(entry) {
            Ok(()) => {}
            Err(PreparationError::Unsupported) => {
                saw_unsupported = true;
                continue;
            }
            Err(other) => return Err(other),
        }
        saw_decodable = true;
        let lfh_offset =
            usize::try_from(entry.lfh_offset).map_err(|_| PreparationError::ArchiveDamaged)?;
        let check_byte = zipcrypto_check_byte(data, lfh_offset, entry);
        if let Some(unit) = select_zip_encrypted_entry(index, entry, payload, check_byte) {
            return Ok(unit);
        }
    }
    if saw_decodable {
        Err(PreparationError::NoCredential)
    } else if saw_unsupported {
        Err(PreparationError::Unsupported)
    } else {
        Err(PreparationError::NoEncryptedUnit)
    }
}

/// Selects one named ZIP entry as the confirmation target.
///
/// The automatic selection takes the first entry that carries a credential. A caller
/// that wants to confirm against a different entry can name it here.
pub(crate) fn select_zip_unit_at(
    zip: &ZipArchive,
    data: &[u8],
    index: usize,
) -> Result<SelectedUnit, PreparationError> {
    let Some(entry) = zip.entries().get(index) else {
        return Err(PreparationError::NoEncryptedUnit);
    };
    if entry.is_directory || !entry.is_encrypted {
        return Err(PreparationError::NoEncryptedUnit);
    }
    let payload = zip_payload(data, entry)?;
    check_zip_entry_support(entry)?;
    let lfh_offset =
        usize::try_from(entry.lfh_offset).map_err(|_| PreparationError::ArchiveDamaged)?;
    let check_byte = zipcrypto_check_byte(data, lfh_offset, entry);
    select_zip_encrypted_entry(index, entry, payload, check_byte)
        .ok_or(PreparationError::NoCredential)
}

/// Separates valid unsupported encryption from damaged AES metadata.
/// The extraction path currently derives only AES-256 keys; feeding a valid AES-128/192
/// payload into it would turn an unsupported unit into a false candidate rejection.
fn check_zip_entry_support(entry: &ZipEntry) -> Result<(), PreparationError> {
    if entry.compression_method == 99 {
        match entry.aes_strength {
            1 | 2 => return Err(PreparationError::Unsupported),
            3 => {}
            _ => return Err(PreparationError::ArchiveDamaged),
        }
    }
    if !is_decodable_method(entry.actual_method) {
        return Err(PreparationError::Unsupported);
    }
    Ok(())
}

/// Reports whether the archive engine can decode an entry of that compression method.
fn is_decodable_method(actual_method: u16) -> bool {
    matches!(actual_method, 0 | 8 | 93)
}

/// Builds the confirmation unit of one encrypted ZIP entry, if it carries a credential.
fn select_zip_encrypted_entry(
    index: usize,
    entry: &ZipEntry,
    payload: &[u8],
    check_byte: u8,
) -> Option<SelectedUnit> {
    if entry.compression_method == 99 {
        return select_winzip_aes_entry(index, payload);
    }
    select_zipcrypto_entry(index, entry, payload, check_byte)
}

/// Builds the confirmation unit of a WinZip AES entry.
fn select_winzip_aes_entry(index: usize, payload: &[u8]) -> Option<SelectedUnit> {
    if payload.len() < WINZIP_AES_MIN_PAYLOAD {
        return None;
    }
    let salt = payload[..16].to_vec();
    let stored_pvv = [payload[16], payload[17]];
    Some(SelectedUnit {
        target: ConfirmationTarget::ZipEntry { index },
        credential: IntegrityCredential::WinZipAesAuthentication,
        screening: Some(ScreeningMaterial::WinZipAes { salt, stored_pvv }),
    })
}

/// Builds the confirmation unit of a traditional ZipCrypto entry.
///
/// The ZIP CRC field is a mandatory 32-bit value, not an optional one: a zero value
/// is a perfectly valid CRC32 and must never be read as "this entry carries no
/// checksum". The credential of a ZipCrypto entry is the CRC32 the archive publishes
/// for it, and the engine compares the CRC32 of the decrypted plaintext against that
/// value for every entry that carries content.
fn select_zipcrypto_entry(
    index: usize,
    entry: &ZipEntry,
    payload: &[u8],
    check_byte: u8,
) -> Option<SelectedUnit> {
    // An entry without content is the only ZipCrypto case that carries no evidence:
    // the reader short-circuits the encryption check for it and accepts any
    // password. An entry without an encryption header is skipped for the same
    // reason.
    if payload.len() < ZIPCRYPTO_MIN_PAYLOAD || entry.uncompressed_size == 0 {
        return None;
    }
    let header: [u8; 12] = payload[..ZIPCRYPTO_MIN_PAYLOAD].try_into().ok()?;
    Some(SelectedUnit {
        target: ConfirmationTarget::ZipEntry { index },
        credential: IntegrityCredential::EntryCrc32 {
            expected: entry.crc32,
        },
        screening: Some(ScreeningMaterial::ZipCrypto { header, check_byte }),
    })
}

/// Derives the check byte the archive reader expects for a ZipCrypto entry.
///
/// The reader compares against the CRC high byte, unless the entry uses a data
/// descriptor, in which case it compares against the MS-DOS time high byte taken
/// from the local file header. This mirrors the reader exactly, so the screening
/// material stays consistent with the content-level confirmation.
fn zipcrypto_check_byte(data: &[u8], lfh_offset: usize, entry: &ZipEntry) -> u8 {
    let lfh_flag = u16::from_le_bytes([data[lfh_offset + 6], data[lfh_offset + 7]]);
    let lfh_dos_time = u16::from_le_bytes([data[lfh_offset + 10], data[lfh_offset + 11]]);
    if (entry.flag | lfh_flag) & 0x0008 != 0 {
        ((lfh_dos_time >> 8) & 0xFF) as u8
    } else {
        ((entry.crc32 >> 24) & 0xFF) as u8
    }
}

/// Returns the raw encrypted payload of a ZIP entry after a structural bounds check.
fn zip_payload<'a>(data: &'a [u8], entry: &ZipEntry) -> Result<&'a [u8], PreparationError> {
    let lfh_offset =
        usize::try_from(entry.lfh_offset).map_err(|_| PreparationError::ArchiveDamaged)?;
    let (payload_offset, _) =
        parse_local_file_header(data, lfh_offset).map_err(|_| PreparationError::ArchiveDamaged)?;
    if entry.compression_method == 99 {
        // The parsed LFH already bounds its complete extra list. Reuse the reader's
        // exact consistency rule before any candidate can be rejected on content.
        let name_len = u16::from_le_bytes([data[lfh_offset + 26], data[lfh_offset + 27]]) as usize;
        let extra_start = lfh_offset + 30 + name_len;
        crate::zip::reader_stream::validate_lfh_aes_extra(entry, &data[extra_start..payload_offset])
            .map_err(|_| PreparationError::ArchiveDamaged)?;
    }
    let comp_size =
        usize::try_from(entry.compressed_size).map_err(|_| PreparationError::ArchiveDamaged)?;
    let end = payload_offset
        .checked_add(comp_size)
        .ok_or(PreparationError::ArchiveDamaged)?;
    if end > data.len() {
        return Err(PreparationError::ArchiveDamaged);
    }
    Ok(&data[payload_offset..end])
}

/// Selects the 7z entry that may act as the confirmation target.
///
/// The encryption fact is taken from the entry's own folder, never from the global
/// `is_encrypted` header flag: a 7z archive may mix encrypted and unencrypted
/// folders, and only the encrypted one carries password evidence.
pub(crate) fn select_seven_zip_unit(
    archive: &SevenZArchive,
) -> Result<SelectedUnit, PreparationError> {
    let info = archive.info();
    let mut saw_encrypted = false;
    for (index, loc) in archive.seek_index().entries.iter().enumerate() {
        if !is_seven_zip_candidate(info, loc) {
            continue;
        }
        saw_encrypted = true;
        if let Some(unit) = judge_seven_zip_entry(index, loc) {
            return Ok(unit);
        }
    }
    Err(if saw_encrypted {
        PreparationError::NoCredential
    } else {
        PreparationError::NoEncryptedUnit
    })
}

/// Selects one named 7z entry as the confirmation target.
///
/// The automatic selection takes the first entry that carries a credential. A caller
/// that wants to confirm against a different entry — for example one that sits behind
/// a large preceding decode inside a solid folder — can name it here.
pub(crate) fn select_seven_zip_unit_at(
    archive: &SevenZArchive,
    index: usize,
) -> Result<SelectedUnit, PreparationError> {
    let info = archive.info();
    let Some(loc) = archive.seek_index().get_by_index(index) else {
        return Err(PreparationError::NoEncryptedUnit);
    };
    if !is_seven_zip_candidate(info, loc) {
        return Err(PreparationError::NoEncryptedUnit);
    }
    judge_seven_zip_entry(index, loc).ok_or(PreparationError::NoCredential)
}

/// Reports whether a 7z entry could carry a password proof at all.
fn is_seven_zip_candidate(info: &SevenZHeaderInfo, loc: &SevenZEntryLocation) -> bool {
    if loc.is_directory || loc.is_empty_stream || loc.uncompressed_size == 0 {
        return false;
    }
    let Some(folder_index) = loc.folder_index else {
        return false;
    };
    let Some(folder) = info.folders.get(folder_index) else {
        return false;
    };
    folder.coders.iter().any(|coder| coder.method_id == METHOD_AES)
}

/// Builds the confirmation unit of one 7z entry that sits in an encrypted folder.
///
/// The credential is the CRC the engine actually compares the extracted bytes
/// against, which is the CRC of the entry's own unpacked stream.
fn judge_seven_zip_entry(index: usize, loc: &SevenZEntryLocation) -> Option<SelectedUnit> {
    let folder_index = loc.folder_index?;
    let expected = loc.crc?;
    Some(SelectedUnit {
        target: ConfirmationTarget::SevenZipEntry {
            index,
            folder_index,
        },
        credential: IntegrityCredential::EntryCrc32 { expected },
        screening: None,
    })
}
