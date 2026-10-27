// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Format standards compliance and 16-format magic sniffing subsystem.

pub use ttzip_archive::standards::anchors;
pub mod checkers;
#[cfg(feature = "probe")]
pub mod document_stream;
pub mod extra_fields;
pub mod ffi;
#[cfg(feature = "probe")]
pub mod image_pipeline;
pub mod report;
pub use ttzip_archive::standards::signatures;
pub use ttzip_archive::standards::sniffer;

pub use anchors::Anchor;
pub use checkers::{check_compliance_buffer, check_compliance_file};
#[cfg(feature = "probe")]
pub use document_stream::*;
pub use extra_fields::{ParsedExtraFields, RawExtraField, RawExtraFieldsIter};
pub use ffi::*;
#[cfg(feature = "probe")]
pub use image_pipeline::*;
pub use report::{ComplianceIssue, ComplianceReport, ComplianceSeverity, ComplianceStandard, StandardCitation};
pub use signatures::{CompoundFormat, DetectedFormat, SignatureEntry, PRIORITIZED_SIGNATURES};
pub use sniffer::{detect_format_buffer, detect_format_file, SniffResult};
