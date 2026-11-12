// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! RFC 8878 Zstandard (zstd) format standards compliance checker.

use crate::standards::report::{ComplianceReport, ComplianceStandard, StandardCitation};
use crate::standards::signatures::DetectedFormat;

const ZSTD_MAGIC: u32 = 0xFD2FB528;
const ZSTD_SKIPPABLE_START: u32 = 0x184D2A50;
const ZSTD_SKIPPABLE_END: u32 = 0x184D2A5F;

pub fn check_zstd_compliance(buffer: &[u8]) -> ComplianceReport {
    let mut report = ComplianceReport::new(DetectedFormat::Zstd);

    if buffer.len() < 4 {
        let citation = StandardCitation::new(ComplianceStandard::Rfc8878Zstd, "3.1.1", "Magic Number");
        report.add_error(citation, "RFC 8878: Zstandard stream truncated before 4-byte magic number", Some(0));
        return report;
    }

    let magic = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
    if (ZSTD_SKIPPABLE_START..=ZSTD_SKIPPABLE_END).contains(&magic) {
        validate_skippable_zstd_frame(buffer, magic, &mut report);
        return report;
    }

    if magic != ZSTD_MAGIC {
        let citation = StandardCitation::new(ComplianceStandard::Rfc8878Zstd, "3.1.1", "Zstandard Magic Number");
        report.add_error(citation, format!("RFC 8878: Invalid Zstandard Frame Magic Number (got 0x{:08X})", magic), Some(0));
        return report;
    }
    report.add_validated_header("RFC 8878: Zstandard Frame Magic Number (0xFD2FB528)");

    if let Some(cursor) = parse_zstd_frame_header(buffer, &mut report) {
        validate_first_zstd_block_header(buffer, cursor, &mut report);
    }

    report
}

fn validate_skippable_zstd_frame(buffer: &[u8], magic: u32, report: &mut ComplianceReport) {
    report.add_validated_header(format!("RFC 8878: Zstandard Skippable Frame Magic (0x{:08X})", magic));
    report.add_metadata("frame_type", "skippable");
    if buffer.len() < 8 {
        let citation = StandardCitation::new(ComplianceStandard::Rfc8878Zstd, "3.1.2", "Skippable Frame");
        report.add_error(citation, "RFC 8878: Truncated skippable frame size header", Some(4));
        return;
    }
    let frame_size = u32::from_le_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]);
    report.add_metadata("skippable_frame_size", frame_size.to_string());
}

fn parse_zstd_frame_header(buffer: &[u8], report: &mut ComplianceReport) -> Option<usize> {
    if buffer.len() < 5 {
        let citation = StandardCitation::new(ComplianceStandard::Rfc8878Zstd, "3.1.1.1", "Frame Header Descriptor");
        report.add_error(citation, "RFC 8878: Buffer truncated before Frame_Header_Descriptor byte", Some(4));
        return None;
    }

    let fhd = buffer[4];
    let dict_id_flag = fhd & 0x03;
    let checksum_flag = (fhd >> 2) & 0x01 != 0;
    let reserved_bit = (fhd >> 3) & 0x01 != 0;
    let single_segment_flag = (fhd >> 5) & 0x01 != 0;
    let fcs_flag = (fhd >> 6) & 0x03;

    if reserved_bit {
        let citation = StandardCitation::new(ComplianceStandard::Rfc8878Zstd, "3.1.1.1.1", "FHD Reserved Bit");
        report.add_error(citation, "RFC 8878: Frame Header Descriptor reserved bit 3 is set", Some(4));
    }
    report.add_validated_header("RFC 8878: Frame Header Descriptor (FHD)");
    report.add_metadata("has_checksum", checksum_flag.to_string());
    report.add_metadata("single_segment", single_segment_flag.to_string());

    let mut cursor = 5;
    if !single_segment_flag {
        if cursor >= buffer.len() {
            let citation = StandardCitation::new(ComplianceStandard::Rfc8878Zstd, "3.1.1.1.2", "Window Descriptor");
            report.add_error(citation, "RFC 8878: Truncated Window_Descriptor byte", Some(cursor as u64));
            return None;
        }
        report.add_validated_header("RFC 8878: Window Descriptor (Window_Size)");
        cursor += 1;
    }

    let dict_bytes = [0usize, 1, 2, 4][(dict_id_flag & 0x03) as usize];
    if cursor + dict_bytes > buffer.len() {
        let citation = StandardCitation::new(ComplianceStandard::Rfc8878Zstd, "3.1.1.1.3", "Dictionary ID");
        report.add_error(citation, "RFC 8878: Truncated Dictionary_ID field", Some(cursor as u64));
        return None;
    }
    cursor += dict_bytes;

    let fcs_bytes = match fcs_flag {
        0 => usize::from(single_segment_flag),
        1 => 2,
        2 => 4,
        _ => 8,
    };
    if cursor + fcs_bytes > buffer.len() {
        let citation = StandardCitation::new(ComplianceStandard::Rfc8878Zstd, "3.1.1.1.4", "Frame Content Size");
        report.add_error(citation, "RFC 8878: Truncated Frame_Content_Size field", Some(cursor as u64));
        return None;
    }

    Some(cursor + fcs_bytes)
}

fn validate_first_zstd_block_header(buffer: &[u8], cursor: usize, report: &mut ComplianceReport) {
    if cursor + 3 > buffer.len() {
        return;
    }
    let block_hdr = u32::from_le_bytes([buffer[cursor], buffer[cursor + 1], buffer[cursor + 2], 0]);
    let block_type = (block_hdr >> 1) & 0x03;
    let block_size = block_hdr >> 3;

    if block_type == 3 {
        let citation = StandardCitation::new(ComplianceStandard::Rfc8878Zstd, "3.1.1.2", "Block Type Reserved");
        report.add_error(citation, "RFC 8878: Block Header contains reserved Block_Type 3", Some(cursor as u64));
    } else {
        report.add_validated_header("RFC 8878: Block Header (Last_Block and Block_Type)");
    }

    if block_size > 131072 {
        let citation = StandardCitation::new(ComplianceStandard::Rfc8878Zstd, "3.1.1.2", "Block Size Upper Bound");
        report.add_warning(
            citation,
            format!("Block size {} exceeds standard 128KB threshold", block_size),
            Some(cursor as u64),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_zstd_compliance() {
        let mut buf = vec![0u8; 16];
        buf[0..4].copy_from_slice(&ZSTD_MAGIC.to_le_bytes());
        buf[4] = 0x20; // Single_Segment_Flag = 1, FCS_Flag = 0 (1 byte size)
        buf[5] = 0x00; // FCS = 0

        let report = check_zstd_compliance(&buf);
        assert!(report.is_compliant, "Valid minimal Zstandard frame should pass compliance");
    }
}
