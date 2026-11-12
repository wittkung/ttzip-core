// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! RFC 1952 GZIP format standards compliance checker.

use crate::standards::report::{ComplianceReport, ComplianceStandard, StandardCitation};
use crate::standards::signatures::DetectedFormat;

const GZIP_ID1: u8 = 0x1F;
const GZIP_ID2: u8 = 0x8B;
const CM_DEFLATE: u8 = 8;

const _FLAG_FTEXT: u8 = 1 << 0;
const FLAG_FHCRC: u8 = 1 << 1;
const FLAG_FEXTRA: u8 = 1 << 2;
const FLAG_FNAME: u8 = 1 << 3;
const FLAG_FCOMMENT: u8 = 1 << 4;
const FLAG_RESERVED: u8 = 0xE0; // Bits 5, 6, 7

pub fn check_gzip_compliance(buffer: &[u8]) -> ComplianceReport {
    let mut report = ComplianceReport::new(DetectedFormat::Gzip);

    let Some(flags) = validate_gzip_fixed_header(buffer, &mut report) else {
        return report;
    };

    if !traverse_gzip_optional_fields(buffer, flags, &mut report) {
        return report;
    }

    verify_gzip_trailer(buffer, &mut report);
    report
}

fn validate_gzip_fixed_header(buffer: &[u8], report: &mut ComplianceReport) -> Option<u8> {
    if buffer.len() < 10 {
        let citation = StandardCitation::new(ComplianceStandard::Rfc1952Gzip, "2.3.1", "Member Header Size");
        report.add_error(
            citation,
            format!("RFC 1952: GZIP stream truncated before 10-byte header ({} bytes)", buffer.len()),
            Some(0),
        );
        return None;
    }

    if buffer[0] == GZIP_ID1 && buffer[1] == GZIP_ID2 {
        report.add_validated_header("RFC 1952: GZIP Member ID1/ID2 Header Magic (0x1F8B)");
    } else {
        let citation = StandardCitation::new(ComplianceStandard::Rfc1952Gzip, "2.3.1", "ID1 and ID2 Identification");
        report.add_error(
            citation,
            format!("RFC 1952: Invalid GZIP magic header (expected 0x1F8B, got 0x{:02X}{:02X})", buffer[0], buffer[1]),
            Some(0),
        );
        return None;
    }

    let cm = buffer[2];
    if cm == CM_DEFLATE {
        report.add_validated_header("RFC 1952: Compression Method DEFLATE (CM=8)");
    } else {
        let citation = StandardCitation::new(ComplianceStandard::Rfc1952Gzip, "2.3.1", "Compression Method");
        report.add_error(
            citation,
            format!("RFC 1952: Unsupported GZIP compression method ID: {} (expected 8 for DEFLATE)", cm),
            Some(2),
        );
    }

    let flags = buffer[3];
    if (flags & FLAG_RESERVED) != 0 {
        let citation = StandardCitation::new(ComplianceStandard::Rfc1952Gzip, "2.3.1", "Reserved Flags");
        report.add_error(
            citation,
            format!("RFC 1952: Reserved flag bits 5-7 must be zero (got 0x{:02X})", flags & FLAG_RESERVED),
            Some(3),
        );
    }
    report.add_validated_header("RFC 1952: Header Flags and MTIME Specification");

    let mtime = u32::from_le_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]);
    report.add_metadata("mtime", mtime.to_string());
    report.add_metadata("xfl", buffer[8].to_string());
    report.add_metadata("os", buffer[9].to_string());

    Some(flags)
}

fn traverse_gzip_optional_fields(buffer: &[u8], flags: u8, report: &mut ComplianceReport) -> bool {
    let mut cursor = 10;

    if (flags & FLAG_FEXTRA) != 0 {
        let Some(next) = skip_gzip_fextra(buffer, cursor, report) else {
            return false;
        };
        cursor = next;
    }

    if (flags & FLAG_FNAME) != 0 {
        let Some(next) = skip_null_terminated_field(
            buffer,
            cursor,
            "Original File Name",
            "RFC 1952: Unterminated GZIP original filename string",
            "RFC 1952: FNAME Original Filename Header",
            report,
        ) else {
            return false;
        };
        cursor = next;
    }

    if (flags & FLAG_FCOMMENT) != 0 {
        let Some(next) = skip_null_terminated_field(
            buffer,
            cursor,
            "File Comment",
            "RFC 1952: Unterminated GZIP comment string",
            "RFC 1952: FCOMMENT File Comment Header",
            report,
        ) else {
            return false;
        };
        cursor = next;
    }

    if (flags & FLAG_FHCRC) != 0 {
        if cursor + 2 > buffer.len() {
            let citation = StandardCitation::new(ComplianceStandard::Rfc1952Gzip, "2.3.1", "Header CRC16");
            report.add_error(citation, "RFC 1952: Truncated GZIP header CRC16 field", Some(cursor as u64));
            return false;
        }
        report.add_validated_header("RFC 1952: FHCRC Header CRC16 Checksum");
    }

    true
}

fn skip_gzip_fextra(buffer: &[u8], cursor: usize, report: &mut ComplianceReport) -> Option<usize> {
    if cursor + 2 > buffer.len() {
        let citation = StandardCitation::new(ComplianceStandard::Rfc1952Gzip, "2.3.1.1", "Extra Field Length");
        report.add_error(citation, "RFC 1952: Truncated GZIP XLEN field", Some(cursor as u64));
        return None;
    }
    let xlen = u16::from_le_bytes([buffer[cursor], buffer[cursor + 1]]) as usize;
    let payload_start = cursor + 2;
    if payload_start + xlen > buffer.len() {
        let citation = StandardCitation::new(ComplianceStandard::Rfc1952Gzip, "2.3.1.1", "Extra Field Payload");
        report.add_error(
            citation,
            "RFC 1952: GZIP extra field extends beyond buffer boundary",
            Some(payload_start as u64),
        );
        return None;
    }
    report.add_validated_header("RFC 1952: FEXTRA Header Extension Block");
    Some(payload_start + xlen)
}

fn skip_null_terminated_field(
    buffer: &[u8],
    cursor: usize,
    section_desc: &'static str,
    err_msg: &'static str,
    validated_hdr: &'static str,
    report: &mut ComplianceReport,
) -> Option<usize> {
    match buffer.get(cursor..)?.iter().position(|&b| b == 0) {
        Some(pos) => {
            report.add_validated_header(validated_hdr);
            Some(cursor + pos + 1)
        }
        None => {
            let citation = StandardCitation::new(ComplianceStandard::Rfc1952Gzip, "2.3.1", section_desc);
            report.add_error(citation, err_msg, Some(cursor as u64));
            None
        }
    }
}

fn verify_gzip_trailer(buffer: &[u8], report: &mut ComplianceReport) {
    if buffer.len() >= 18 {
        report.add_validated_header("RFC 1952: Trailer CRC32 and ISIZE Fields (offset EOF-8)");
        let trailer_start = buffer.len() - 8;
        if let (Some(crc_slice), Some(isize_slice)) = (
            buffer.get(trailer_start..trailer_start + 4),
            buffer.get(trailer_start + 4..trailer_start + 8),
        ) {
            if let (Ok(crc_bytes), Ok(isize_bytes)) = (crc_slice.try_into(), isize_slice.try_into()) {
                let crc32_val = u32::from_le_bytes(crc_bytes);
                let isize_val = u32::from_le_bytes(isize_bytes);
                report.add_metadata("trailer_crc32", format!("0x{:08X}", crc32_val));
                report.add_metadata("trailer_isize", isize_val.to_string());
            }
        }
    } else {
        let citation = StandardCitation::new(ComplianceStandard::Rfc1952Gzip, "2.3.1", "Member Trailer");
        report.add_warning(
            citation,
            "RFC 1952: Stream too short to contain full trailer CRC32 and ISIZE fields",
            Some(buffer.len() as u64),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_gzip_compliance() {
        let mut buf = vec![0u8; 18];
        buf[0] = GZIP_ID1;
        buf[1] = GZIP_ID2;
        buf[2] = CM_DEFLATE;
        buf[3] = 0; // No optional flags
        buf[8] = 2; // Maximum compression
        buf[9] = 3; // Unix OS

        let report = check_gzip_compliance(&buf);
        assert!(report.is_compliant, "Valid minimal GZIP should pass compliance");
    }
}
