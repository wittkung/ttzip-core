// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Format compliance checker registry and dispatch engine.

pub mod bzip2;
pub mod disk_images;
pub mod gzip;
pub mod modern;
pub mod sevenz;
pub mod tar;
pub mod xz;
pub mod zip;
pub mod zstd;

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use super::report::{ComplianceReport, ComplianceStandard, StandardCitation};
use super::signatures::DetectedFormat;
use super::sniffer::{detect_format_buffer, detect_format_file};

type ComplianceCheckerFn = fn(&[u8]) -> ComplianceReport;

const COMPLIANCE_CHECKER_TABLE: &[(DetectedFormat, ComplianceCheckerFn)] = &[
    (DetectedFormat::Zip, zip::check_zip_compliance),
    (DetectedFormat::Tar, tar::check_tar_compliance),
    (DetectedFormat::SevenZip, sevenz::check_sevenz_compliance),
    (DetectedFormat::Gzip, gzip::check_gzip_compliance),
    (DetectedFormat::Zstd, zstd::check_zstd_compliance),
    (DetectedFormat::Bzip2, bzip2::check_bzip2_compliance),
    (DetectedFormat::Xz, xz::check_xz_compliance),
    (DetectedFormat::Dmg, disk_images::check_dmg_compliance),
    (DetectedFormat::Iso, disk_images::check_iso_compliance),
    (DetectedFormat::Wim, disk_images::check_wim_compliance),
    (DetectedFormat::Xar, modern::check_xar_compliance),
    (DetectedFormat::Lzfse, modern::check_lzfse_compliance),
    (DetectedFormat::Snappy, modern::check_snappy_compliance),
    (DetectedFormat::Lz4, modern::check_lz4_compliance),
    (DetectedFormat::Lzip, modern::check_lzip_compliance),
    (DetectedFormat::Lrzip, modern::check_lrzip_compliance),
    (DetectedFormat::Aar, modern::check_aar_compliance),
    (DetectedFormat::Brotli, modern::check_brotli_compliance),
    (DetectedFormat::Ar, modern::check_ar_compliance),
    (DetectedFormat::Rar, modern::check_rar_compliance),
    (DetectedFormat::Cab, modern::check_cab_compliance),
];

/// Dispatches buffer-level compliance inspection according to detected or specified format.
pub fn check_compliance_buffer(format: DetectedFormat, buffer: &[u8]) -> ComplianceReport {
    let resolved_format = if format == DetectedFormat::Unknown {
        detect_format_buffer(buffer, None).format
    } else {
        format
    };

    if let Some((_, checker)) = COMPLIANCE_CHECKER_TABLE
        .iter()
        .find(|(fmt, _)| *fmt == resolved_format)
    {
        return checker(buffer);
    }

    let mut report = ComplianceReport::new(DetectedFormat::Unknown);
    let citation = StandardCitation::new(ComplianceStandard::Generic, "1.0", "Format Identification");
    report.add_error(
        citation,
        "Unknown or unsupported format for compliance checking",
        Some(0),
    );
    report
}

/// Inspects and verifies compliance of an on-disk archive file.
pub fn check_compliance_file<P: AsRef<Path>>(path: P) -> io::Result<ComplianceReport> {
    let sniff_result = detect_format_file(&path)?;
    let mut file = File::open(&path)?;
    let file_len = file.metadata()?.len() as usize;

    // Read full file if small, or up to 4MB inspection window
    let read_len = file_len.min(4 * 1024 * 1024);
    let mut buffer = vec![0u8; read_len];
    file.read_exact(&mut buffer)?;

    Ok(check_compliance_buffer(sniff_result.format, &buffer))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispatch_gzip_compliance() {
        let mut gz_buf = vec![0u8; 18];
        gz_buf[0] = 0x1F;
        gz_buf[1] = 0x8B;
        gz_buf[2] = 8;
        let report = check_compliance_buffer(DetectedFormat::Unknown, &gz_buf);
        assert_eq!(report.format, DetectedFormat::Gzip);
        assert!(report.is_compliant);
    }
}
