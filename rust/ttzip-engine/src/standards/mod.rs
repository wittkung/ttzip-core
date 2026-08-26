// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Format standards compliance and 16-format magic sniffing subsystem.

pub mod anchors;
pub mod checkers;
#[cfg(feature = "probe")]
pub mod document_stream;
pub mod extra_fields;
pub mod ffi;
#[cfg(feature = "probe")]
pub mod image_pipeline;
#[cfg(feature = "probe")]
pub mod metadata_probe;
pub mod report;
pub mod signatures;
pub mod sniffer;
pub mod syntax_highlight;

pub use anchors::Anchor;
pub use checkers::{check_compliance_buffer, check_compliance_file};
#[cfg(feature = "probe")]
pub use document_stream::*;
pub use extra_fields::{ParsedExtraFields, RawExtraField, RawExtraFieldsIter};
pub use ffi::*;
#[cfg(feature = "probe")]
pub use image_pipeline::*;
#[cfg(feature = "probe")]
pub use metadata_probe::*;
pub use report::{ComplianceIssue, ComplianceReport, ComplianceSeverity, ComplianceStandard, StandardCitation};
pub use signatures::{CompoundFormat, DetectedFormat, SignatureEntry, PRIORITIZED_SIGNATURES};
pub use sniffer::{detect_format_buffer, detect_format_file, SniffResult};
pub use syntax_highlight::*;
