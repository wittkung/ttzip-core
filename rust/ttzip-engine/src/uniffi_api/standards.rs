// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Mozilla UniFFI Archive Standards Compliance Auditing.
//!
//! Provides deep physical inspection of archive container headers, directory tables,
//! and metadata structures according to official file format specifications.

use std::path::Path;
use super::types::TTZipError;
use crate::standards::checkers::check_compliance_file;
use crate::standards::report::{ComplianceReport, ComplianceSeverity, ComplianceStandard};
use crate::standards::signatures::DetectedFormat;

/// Structured compliance audit report exposed via Mozilla UniFFI.
#[derive(Clone, Debug, uniffi::Record)]
pub struct UniFFIStandardsComplianceReport {
    pub format_name: String,
    pub is_compliant: bool,
    pub standard_citation: String,
    pub validated_headers: Vec<String>,
    pub warnings: Vec<String>,
    pub violations: Vec<String>,
}

impl From<ComplianceReport> for UniFFIStandardsComplianceReport {
    fn from(report: ComplianceReport) -> Self {
        let mut warnings = Vec::new();
        let mut violations = Vec::new();

        for issue in &report.issues {
            let msg = if let Some(offset) = issue.offset {
                format!("{}: {} (offset {})", issue.citation.section, issue.message, offset)
            } else {
                format!("{}: {}", issue.citation.section, issue.message)
            };

            match issue.severity {
                ComplianceSeverity::Error => violations.push(msg),
                ComplianceSeverity::Warning => warnings.push(msg),
                ComplianceSeverity::Notice | ComplianceSeverity::Info => {}
            }
        }

        let standard_citation = if let Some(first_issue) = report.issues.first() {
            if !first_issue.citation.section.is_empty() {
                format!(
                    "{} §{} ({})",
                    first_issue.citation.standard.name(),
                    first_issue.citation.section,
                    first_issue.citation.description
                )
            } else {
                first_issue.citation.standard.name().to_string()
            }
        } else {
            canonical_standard_name(report.format).to_string()
        };

        Self {
            format_name: format!("{:?}", report.format),
            is_compliant: report.is_compliant,
            standard_citation,
            validated_headers: report.validated_headers,
            warnings,
            violations,
        }
    }
}

fn container_compliance_standard(format: DetectedFormat) -> Option<ComplianceStandard> {
    match format {
        DetectedFormat::Zip => Some(ComplianceStandard::PkwareAppnote),
        DetectedFormat::SevenZip => Some(ComplianceStandard::SevenZipSpec),
        DetectedFormat::Tar => Some(ComplianceStandard::Posix1Tar),
        DetectedFormat::Rar => Some(ComplianceStandard::RarSpec),
        DetectedFormat::Cab => Some(ComplianceStandard::CabSpec),
        DetectedFormat::Iso => Some(ComplianceStandard::Iso9660),
        DetectedFormat::Dmg => Some(ComplianceStandard::AppleDmgUdif),
        DetectedFormat::Xar => Some(ComplianceStandard::XarSpec),
        DetectedFormat::Aar => Some(ComplianceStandard::AarSpec),
        DetectedFormat::Wim => Some(ComplianceStandard::WimSpec),
        DetectedFormat::Ar => Some(ComplianceStandard::UnixArSpec),
        _ => None,
    }
}

fn stream_compliance_standard(format: DetectedFormat) -> ComplianceStandard {
    match format {
        DetectedFormat::Gzip => ComplianceStandard::Rfc1952Gzip,
        DetectedFormat::Bzip2 => ComplianceStandard::Bzip2Spec,
        DetectedFormat::Xz => ComplianceStandard::XzSpec,
        DetectedFormat::Zstd => ComplianceStandard::Rfc8878Zstd,
        DetectedFormat::Lzfse => ComplianceStandard::LzfseSpec,
        DetectedFormat::Snappy => ComplianceStandard::SnappySpec,
        DetectedFormat::Lz4 => ComplianceStandard::Lz4Spec,
        DetectedFormat::Lzip => ComplianceStandard::LzipSpec,
        DetectedFormat::Lrzip => ComplianceStandard::LrzipSpec,
        DetectedFormat::Brotli => ComplianceStandard::BrotliSpec,
        _ => ComplianceStandard::Generic,
    }
}

/// Resolves authoritative canonical standard name for a detected archive format.
fn canonical_standard_name(format: DetectedFormat) -> &'static str {
    container_compliance_standard(format)
        .unwrap_or_else(|| stream_compliance_standard(format))
        .name()
}

/// Audits an on-disk archive file against official container and codec standards.
///
/// Performs physical header validation, directory table parsing, and extension field
/// verification without mock stubs.
#[uniffi::export]
pub fn check_archive_standards_compliance(
    archive_path: String,
) -> Result<UniFFIStandardsComplianceReport, TTZipError> {
    let path = Path::new(&archive_path);
    if !path.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    let report = check_compliance_file(path)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;

    Ok(UniFFIStandardsComplianceReport::from(report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_check_archive_standards_compliance_valid_zip() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let zip_file = temp_dir.path().join("test_empty.zip");
        {
            let mut f = std::fs::File::create(&zip_file).expect("create zip");
            let mut eocd = [0u8; 22];
            eocd[0..4].copy_from_slice(b"PK\x05\x06");
            f.write_all(&eocd).expect("write eocd");
        }

        let report = check_archive_standards_compliance(zip_file.to_str().unwrap().to_string()).unwrap();
        assert_eq!(report.format_name, "Zip");
        assert!(report.is_compliant);
        assert!(report.violations.is_empty());
        assert!(!report.validated_headers.is_empty());
        assert!(report.standard_citation.contains("PKWARE APPNOTE"));
    }

    #[test]
    fn test_check_archive_standards_compliance_valid_gzip() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let gz_file = temp_dir.path().join("test.gz");
        {
            let mut f = std::fs::File::create(&gz_file).expect("create gz");
            let mut gz = [0u8; 18];
            gz[0] = 0x1F;
            gz[1] = 0x8B;
            gz[2] = 8; // DEFLATE
            f.write_all(&gz).expect("write gz");
        }

        let report = check_archive_standards_compliance(gz_file.to_str().unwrap().to_string()).unwrap();
        assert_eq!(report.format_name, "Gzip");
        assert!(report.is_compliant);
        assert!(report.violations.is_empty());
        assert!(report.standard_citation.contains("RFC 1952"));
    }

    #[test]
    fn test_check_archive_standards_compliance_missing_file() {
        let err = check_archive_standards_compliance("/path/to/nonexistent/file.zip".to_string());
        assert!(err.is_err());
        match err.unwrap_err() {
            TTZipError::FileNotFound { path } => assert!(path.contains("nonexistent")),
            other => panic!("Expected FileNotFound, got {:?}", other),
        }
    }
}
