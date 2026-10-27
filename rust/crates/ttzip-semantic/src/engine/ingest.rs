// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Per-file incremental freshness checks and path/content segment embedding.

use super::SemanticSearchCoordinator;
use crate::chunker::{
    chunk_text_content, classify_content_tier, extract_safe_utf8_text, is_excluded_relative_path,
    ContentSupportTier, DEFAULT_CHUNK_CHARS, DEFAULT_CHUNK_OVERLAP_CHARS,
};
use crate::index::FileIndexInput;
use crate::model::{validate_non_zero_int8_vector, SemanticModelError};
use std::sync::atomic::{AtomicBool, Ordering};

type BuiltSegment = (bool, u32, u32, u8, String, [i8; 128]);

pub(super) struct FileIngestInput<'a> {
    pub relative_path: &'a str,
    pub archive_entry_path: Option<String>,
    pub bytes: &'a [u8],
    pub mtime_ns: i64,
}

impl SemanticSearchCoordinator {
    pub(super) fn ingest_single_file(
        &self,
        scope_id: &str,
        idx: usize,
        file: FileIngestInput<'_>,
        cancel_flag: &AtomicBool,
    ) -> Result<(), SemanticModelError> {
        let FileIngestInput {
            relative_path,
            archive_entry_path,
            bytes,
            mtime_ns,
        } = file;
        if is_excluded_relative_path(relative_path) {
            return Ok(());
        }
        let file_size = bytes.len() as u64;
        let tier = classify_content_tier(relative_path, file_size.max(1), false);
        if tier == ContentSupportTier::Excluded {
            return Ok(());
        }

        let mut hasher = blake3::Hasher::new();
        hasher.update(relative_path.as_bytes());
        hasher.update(bytes);
        let content_blake3 = *hasher.finalize().as_bytes();

        let already_fresh = {
            let guard = self.scopes.read().expect("scopes lock poisoned");
            guard.get(scope_id).is_some_and(|m| {
                m.index
                    .is_file_up_to_date(relative_path, mtime_ns, file_size, &content_blake3)
            })
        };
        if already_fresh {
            let mut guard = self.scopes.write().expect("scopes lock poisoned");
            if let Some(managed) = guard.get_mut(scope_id) {
                managed.progress.processed_files = (idx + 1) as u32;
            }
            return Ok(());
        }

        let built_segments = self.build_file_segments(relative_path, bytes, tier, cancel_flag)?;

        let mut guard = self.scopes.write().expect("scopes lock poisoned");
        if let Some(managed) = guard.get_mut(scope_id) {
            managed.index.upsert_file_segments(
                FileIndexInput {
                    relative_path: relative_path.to_string(),
                    archive_entry_path,
                    mtime_ns,
                    file_size,
                    content_blake3,
                    tier,
                },
                built_segments,
            )?;
            managed.progress.processed_files = (idx + 1) as u32;
            managed.progress.indexed_segments = managed.index.segments().len() as u32;
            managed.progress.index_generation = managed.index.index_generation;
        }
        Ok(())
    }

    fn build_file_segments(
        &self,
        relative_path: &str,
        bytes: &[u8],
        tier: ContentSupportTier,
        cancel_flag: &AtomicBool,
    ) -> Result<Vec<BuiltSegment>, SemanticModelError> {
        let mut built_segments = Vec::new();
        let path_vec = self.provider.encode_document(None, relative_path)?;
        validate_non_zero_int8_vector(&path_vec)?;
        built_segments.push((true, 0, 0, 0, relative_path.to_string(), path_vec));

        if matches!(
            tier,
            ContentSupportTier::Tier2PlainText | ContentSupportTier::Tier2SourceCode
        ) {
            if let Some(text) = extract_safe_utf8_text(bytes) {
                let is_code = tier == ContentSupportTier::Tier2SourceCode;
                let chunks = chunk_text_content(
                    &text,
                    is_code,
                    DEFAULT_CHUNK_CHARS,
                    DEFAULT_CHUNK_OVERLAP_CHARS,
                );
                for ch in chunks {
                    if cancel_flag.load(Ordering::SeqCst) {
                        break;
                    }
                    let vec_i8 = self
                        .provider
                        .encode_document(Some(relative_path), &ch.text)?;
                    validate_non_zero_int8_vector(&vec_i8)?;
                    let preview = ch.text.chars().take(240).collect::<String>();
                    built_segments.push((
                        false,
                        ch.line_start,
                        ch.line_end,
                        ch.definition_strength,
                        preview,
                        vec_i8,
                    ));
                }
            }
        }

        Ok(built_segments)
    }
}
