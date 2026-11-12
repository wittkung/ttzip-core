// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! PKWARE APPNOTE .ZIP format standards compliance checker.

use crate::standards::extra_fields::ParsedExtraFields;
use crate::standards::report::{ComplianceReport, ComplianceStandard, StandardCitation};
use crate::standards::signatures::DetectedFormat;

const SIG_LFH: u32 = 0x04034b50;
const SIG_CDFH: u32 = 0x02014b50;
const _SIG_EOCD: u32 = 0x06054b50;
const SIG_ZIP64_EOCD: u32 = 0x06064b50;
const SIG_ZIP64_LOCATOR: u32 = 0x07064b50;

pub fn check_zip_compliance(buffer: &[u8]) -> ComplianceReport {
    let mut report = ComplianceReport::new(DetectedFormat::Zip);

    if buffer.len() < 22 {
        let citation = StandardCitation::new(ComplianceStandard::PkwareAppnote, "4.4.1", "End of Central Directory Record");
        report.add_error(citation, "ZIP file smaller than minimum EOCD record size (22 bytes)", Some(0));
        return report;
    }

    validate_offset0_signature(buffer, &mut report);

    let Some(eocd_pos) = find_eocd(buffer) else {
        let citation = StandardCitation::new(ComplianceStandard::PkwareAppnote, "4.4.1", "End of Central Directory Record");
        report.add_error(citation, "PKWARE APPNOTE: Missing End of Central Directory (EOCD) record (0x06054B50)", None);
        return report;
    };
    report.add_validated_header("PKWARE APPNOTE: End of Central Directory Record (0x06054B50)");

    let (resolved_cd_offset, resolved_cd_entries) =
        parse_eocd_and_zip64(buffer, eocd_pos, &mut report);

    if resolved_cd_offset as usize > buffer.len() {
        let citation = StandardCitation::new(ComplianceStandard::PkwareAppnote, "4.4.1", "Central Directory Structure");
        report.add_error(citation, "Central Directory offset exceeds buffer boundary", Some(resolved_cd_offset));
        return report;
    }

    traverse_central_directory(buffer, resolved_cd_offset as usize, resolved_cd_entries, &mut report);
    report
}

fn validate_offset0_signature(buffer: &[u8], report: &mut ComplianceReport) {
    let sig0 = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
    match sig0 {
        0x04034B50 => report.add_validated_header("PKWARE APPNOTE: Local File Header Signature (0x04034B50)"),
        0x06054B50 => report.add_validated_header("PKWARE APPNOTE: Empty Archive EOCD Signature (0x06054B50)"),
        0x08074B50 => report.add_validated_header("PKWARE APPNOTE: Spanned Archive Data Descriptor (0x08074B50)"),
        _ => {
            let citation = StandardCitation::new(ComplianceStandard::PkwareAppnote, "4.3.7", "Local File Header Magic");
            report.add_error(
                citation,
                "PKWARE APPNOTE: Missing valid ZIP Local File Header (0x04034B50) or EOCD (0x06054B50) at offset 0",
                Some(0),
            );
        }
    }
}

fn parse_eocd_and_zip64(
    buffer: &[u8],
    eocd_pos: usize,
    report: &mut ComplianceReport,
) -> (u64, u64) {
    let eocd = &buffer[eocd_pos..];
    let cd_entries_disk = u16::from_le_bytes([eocd[8], eocd[9]]);
    let cd_entries_total = u16::from_le_bytes([eocd[10], eocd[11]]);
    let cd_size = u32::from_le_bytes([eocd[12], eocd[13], eocd[14], eocd[15]]) as usize;
    let cd_offset = u32::from_le_bytes([eocd[16], eocd[17], eocd[18], eocd[19]]) as usize;
    let comment_len = u16::from_le_bytes([eocd[20], eocd[21]]) as usize;

    if cd_entries_disk != 0 {
        let citation = StandardCitation::new(ComplianceStandard::PkwareAppnote, "4.4.1", "Multi-disk Spanned Archive");
        report.add_warning(
            citation,
            format!("Multi-disk ZIP archive detected (disk {})", cd_entries_disk),
            Some(eocd_pos as u64 + 8),
        );
    }

    if eocd_pos + 22 + comment_len > buffer.len() {
        let citation = StandardCitation::new(ComplianceStandard::PkwareAppnote, "4.4.1.5", "Archive Comment Length");
        report.add_error(citation, "EOCD comment length exceeds archive boundary", Some(eocd_pos as u64 + 20));
    }

    report.add_metadata("cd_entries_total", cd_entries_total.to_string());
    report.add_metadata("cd_size", cd_size.to_string());
    report.add_metadata("cd_offset", cd_offset.to_string());

    if eocd_pos >= 20 {
        if let Some(pair) = parse_zip64_locator(buffer, eocd_pos - 20, report) {
            return pair;
        }
    }

    (cd_offset as u64, cd_entries_total as u64)
}

fn parse_zip64_locator(
    buffer: &[u8],
    locator_pos: usize,
    report: &mut ComplianceReport,
) -> Option<(u64, u64)> {
    let sig = u32::from_le_bytes(buffer[locator_pos..locator_pos + 4].try_into().unwrap_or([0u8; 4]));
    if sig != SIG_ZIP64_LOCATOR {
        return None;
    }

    report.add_validated_header("PKWARE APPNOTE: Zip64 End of Central Directory Locator (0x07064B50)");
    let z64_eocd_offset = u64::from_le_bytes(
        buffer[locator_pos + 8..locator_pos + 16]
            .try_into()
            .unwrap_or([0u8; 8]),
    );
    report.add_metadata("has_zip64_locator", "true");
    report.add_metadata("zip64_eocd_offset", z64_eocd_offset.to_string());

    let z64_pos = z64_eocd_offset as usize;
    if z64_pos + 56 > buffer.len() {
        return None;
    }

    let z64_sig = u32::from_le_bytes(buffer[z64_pos..z64_pos + 4].try_into().unwrap_or([0u8; 4]));
    if z64_sig == SIG_ZIP64_EOCD {
        report.add_validated_header("PKWARE APPNOTE: Zip64 End of Central Directory Record (0x06064B50)");
        report.add_validated_header("PKWARE APPNOTE: Archive verified as Zip64 format");
        let entries = u64::from_le_bytes(buffer[z64_pos + 32..z64_pos + 40].try_into().unwrap_or([0u8; 8]));
        let offset = u64::from_le_bytes(buffer[z64_pos + 48..z64_pos + 56].try_into().unwrap_or([0u8; 8]));
        Some((offset, entries))
    } else {
        let citation = StandardCitation::new(ComplianceStandard::PkwareAppnote, "4.3.14", "Zip64 End of Central Directory Record");
        report.add_error(citation, "Zip64 EOCD signature mismatch at locator target offset", Some(z64_eocd_offset));
        None
    }
}

fn traverse_central_directory(
    buffer: &[u8],
    mut cursor: usize,
    resolved_cd_entries: u64,
    report: &mut ComplianceReport,
) {
    let mut parsed_entries = 0u64;
    let mut parsed_cd_header = false;

    while cursor + 46 <= buffer.len() && parsed_entries < resolved_cd_entries {
        let sig = u32::from_le_bytes(buffer[cursor..cursor + 4].try_into().unwrap_or([0u8; 4]));
        if sig != SIG_CDFH {
            break;
        }

        if !parsed_cd_header {
            report.add_validated_header("PKWARE APPNOTE: Central Directory File Header (0x02014B50)");
            parsed_cd_header = true;
        }

        let Some(total_entry_len) = inspect_cdfh_record(buffer, cursor, report) else {
            break;
        };

        cursor += total_entry_len;
        parsed_entries += 1;
    }

    report.add_metadata("verified_cdfh_count", parsed_entries.to_string());
}

fn inspect_cdfh_record(
    buffer: &[u8],
    cursor: usize,
    report: &mut ComplianceReport,
) -> Option<usize> {
    let method = u16::from_le_bytes([buffer[cursor + 10], buffer[cursor + 11]]);
    let comp_size = u32::from_le_bytes(buffer[cursor + 20..cursor + 24].try_into().unwrap_or([0u8; 4]));
    let uncomp_size = u32::from_le_bytes(buffer[cursor + 24..cursor + 28].try_into().unwrap_or([0u8; 4]));
    let fname_len = u16::from_le_bytes([buffer[cursor + 28], buffer[cursor + 29]]) as usize;
    let extra_len = u16::from_le_bytes([buffer[cursor + 30], buffer[cursor + 31]]) as usize;
    let comment_entry_len = u16::from_le_bytes([buffer[cursor + 32], buffer[cursor + 33]]) as usize;
    let local_hdr_offset = u32::from_le_bytes(buffer[cursor + 42..cursor + 46].try_into().unwrap_or([0u8; 4]));

    let total_entry_len = 46 + fname_len + extra_len + comment_entry_len;
    if cursor + total_entry_len > buffer.len() {
        let citation = StandardCitation::new(ComplianceStandard::PkwareAppnote, "4.3.12", "Central Directory Header Size");
        report.add_error(citation, "CDFH record extends beyond buffer length", Some(cursor as u64));
        return None;
    }

    if !matches!(method, 0 | 8 | 12 | 14 | 93 | 99) {
        let citation = StandardCitation::new(ComplianceStandard::PkwareAppnote, "4.4.5", "Compression Methods");
        report.add_warning(citation, format!("Uncommon compression method ID: {}", method), Some(cursor as u64 + 10));
    }

    let extra_data = &buffer[cursor + 46 + fname_len..cursor + 46 + fname_len + extra_len];
    let parsed_extra = ParsedExtraFields::parse(
        extra_data,
        true,
        uncomp_size == 0xFFFFFFFF,
        comp_size == 0xFFFFFFFF,
        local_hdr_offset == 0xFFFFFFFF,
    );
    record_validated_extra_headers(&parsed_extra, report);

    if uncomp_size == 0xFFFFFFFF && parsed_extra.zip64.as_ref().and_then(|z| z.uncompressed_size).is_none() {
        let citation = StandardCitation::new(ComplianceStandard::PkwareAppnote, "4.5.3", "Zip64 Extended Information Extra Field");
        report.add_error(citation, "Zip64 uncompressed size placeholder 0xFFFFFFFF without Zip64 extra field", Some(cursor as u64));
    }

    let target_lfh = parsed_extra
        .zip64
        .as_ref()
        .and_then(|z| z.local_header_offset)
        .unwrap_or(local_hdr_offset as u64);
    verify_lfh_signature(buffer, target_lfh, report);

    Some(total_entry_len)
}

fn record_validated_extra_headers(parsed_extra: &ParsedExtraFields<'_>, report: &mut ComplianceReport) {
    let checks = [
        (parsed_extra.zip64.is_some(), "ZIP Extra Field: Zip64 Extended Information (0x0001)"),
        (parsed_extra.timestamp.is_some(), "ZIP Extra Field: Extended Timestamp (0x5455)"),
        (parsed_extra.unicode_path.is_some(), "ZIP Extra Field: Unicode Path (0x7075)"),
        (parsed_extra.infozip_unix.is_some(), "ZIP Extra Field: Info-ZIP UNIX (0x7875)"),
        (parsed_extra.winzip_aes.is_some(), "ZIP Extra Field: WinZip AES (0x9901)"),
    ];
    for (present, label) in checks {
        if present {
            report.add_validated_header(label);
        }
    }
}

fn verify_lfh_signature(buffer: &[u8], target_lfh: u64, report: &mut ComplianceReport) {
    let pos = target_lfh as usize;
    if pos + 30 <= buffer.len() {
        let lfh_sig = u32::from_le_bytes(buffer[pos..pos + 4].try_into().unwrap_or([0u8; 4]));
        if lfh_sig != SIG_LFH {
            let citation = StandardCitation::new(ComplianceStandard::PkwareAppnote, "4.3.7", "Local File Header Magic");
            report.add_warning(citation, format!("Local header signature mismatch at offset {}", target_lfh), Some(target_lfh));
        }
    }
}

fn find_eocd(buffer: &[u8]) -> Option<usize> {
    let scan_len = buffer.len().min(65557);
    let start = buffer.len() - scan_len;
    let window = &buffer[start..];

    for i in (0..=window.len() - 22).rev() {
        if &window[i..i + 4] == b"PK\x05\x06" {
            let comment_len = u16::from_le_bytes([window[i + 20], window[i + 21]]) as usize;
            if i + 22 + comment_len <= window.len() {
                return Some(start + i);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_zip_compliance() {
        // Minimal valid empty zip file: 22 bytes EOCD
        let mut buf = vec![0u8; 22];
        buf[0..4].copy_from_slice(b"PK\x05\x06");
        let report = check_zip_compliance(&buf);
        assert!(report.is_compliant, "Empty valid zip should be compliant");
    }
}
