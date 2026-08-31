// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zero-panic Type-Length-Value (TLV) Extra Field stream parser.

use super::types::*;
use super::ZipExtraFields;

/// Parser for ZIP Type-Length-Value Extra Fields stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExtraFieldsParser;

impl ExtraFieldsParser {
    /// Parses extra fields from a raw slice.
    #[inline]
    pub fn parse(extra_data: &[u8], is_cdfh: bool) -> ZipExtraFields {
        Self::parse_with_placeholders(extra_data, is_cdfh, true, true, true)
    }

    /// Parses extra fields with explicit placeholder detection flags.
    pub fn parse_with_placeholders(
        extra_data: &[u8],
        is_cdfh: bool,
        uncomp_placeholder: bool,
        comp_placeholder: bool,
        offset_placeholder: bool,
    ) -> ZipExtraFields {
        let mut fields = ZipExtraFields::default();
        if extra_data.len() < 4 {
            return fields;
        }

        let mut offset = 0;
        while offset + 4 <= extra_data.len() {
            let header_id = u16::from_le_bytes([extra_data[offset], extra_data[offset + 1]]);
            let data_size =
                u16::from_le_bytes([extra_data[offset + 2], extra_data[offset + 3]]) as usize;
            let payload_start = offset + 4;

            if payload_start + data_size > extra_data.len() {
                break; // Corrupted or truncated trailing field safely ignored (0 panic)
            }

            let payload = &extra_data[payload_start..payload_start + data_size];
            let total_field_len = 4 + data_size;

            match header_id {
                TAG_ZIP64 => {
                    let z64 = Zip64Extra::parse(
                        payload,
                        is_cdfh,
                        uncomp_placeholder,
                        comp_placeholder,
                        offset_placeholder,
                    );
                    fields.has_zip64 = true;
                    fields.uncompressed_size = z64.uncompressed_size;
                    fields.compressed_size = z64.compressed_size;
                    fields.local_header_offset = z64.local_header_offset;
                    fields.disk_number = z64.disk_start_number;
                    fields.zip64 = Some(z64);
                }

                TAG_WINZIP_AES => {
                    if let Some(aes) = WinZipAesExtra::parse(payload) {
                        fields.has_winzip_aes = true;
                        fields.aes_version = aes.version;
                        fields.aes_vendor_id = aes.vendor_id;
                        fields.aes_strength = aes.strength;
                        fields.aes_actual_method = aes.actual_compression_method;
                        fields.winzip_aes = Some(aes);
                    }
                }

                TAG_EXT_TIMESTAMP => {
                    if let Some(ts) = ExtendedTimestampExtra::parse(payload) {
                        fields.has_extended_timestamp = true;
                        fields.mod_time = ts.mod_time;
                        fields.acc_time = ts.acc_time;
                        fields.create_time = ts.create_time;
                        fields.extended_timestamp = Some(ts);
                    }
                }

                TAG_INFOZIP_UNIX_NEW => {
                    if let Some(ux) = InfoZipUnixNewExtra::parse(payload) {
                        fields.has_posix_permissions = true;
                        fields.uid = Some(ux.uid);
                        fields.gid = Some(ux.gid);
                        fields.infozip_unix_new = Some(ux);
                    }
                }

                TAG_NTFS => {
                    if let Some(ntfs) = NtfsExtra::parse(payload) {
                        fields.ntfs = Some(ntfs);
                    }
                }

                TAG_UNICODE_PATH => {
                    if let Some(upath) = UnicodeFieldExtra::parse(TAG_UNICODE_PATH, payload) {
                        fields.unicode_path_str = Some(upath.text.clone());
                        fields.unicode_path = Some(upath);
                    }
                }

                TAG_UNICODE_COMMENT => {
                    if let Some(ucomm) = UnicodeFieldExtra::parse(TAG_UNICODE_COMMENT, payload) {
                        fields.unicode_comment = Some(ucomm);
                    }
                }

                TAG_ASI_UNIX => {
                    if let Some(asi) = AsiUnixExtra::parse(payload) {
                        fields.asi_unix = Some(asi);
                    }
                }

                TAG_DATA_STREAM_ALIGNMENT => {
                    if let Some(align) =
                        DataStreamAlignmentExtra::parse(payload, total_field_len)
                    {
                        fields.data_stream_alignment = Some(align.alignment);
                        fields.alignment = Some(align);
                    }
                }

                other => {
                    fields.unknown_fields.push((other, payload.to_vec()));
                }
            }

            offset = payload_start + data_size;
        }

        fields
    }
}
