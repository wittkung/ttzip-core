// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Lazy directory bodies: one bounded file payload per ingest step and one commit per run.

use super::ingest::IngestFileEvent;
use super::SemanticSearchCoordinator;
use crate::chunker::{classify_content_tier, ContentSupportTier, MAX_TEXT_FILE_BYTES};
use std::collections::HashSet;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

type DiscoveredFile = (String, PathBuf, u64, i64);

pub(super) struct DirectoryFileEventStream<'a> {
    files: std::vec::IntoIter<DiscoveredFile>,
    removed_paths: std::vec::IntoIter<String>,
    cancel_flag: &'a AtomicBool,
    had_read_error: &'a AtomicBool,
}

impl DirectoryFileEventStream<'_> {
    pub(super) fn remaining_event_count(&self) -> usize {
        self.files.len() + self.removed_paths.len()
    }
}

impl Iterator for DirectoryFileEventStream<'_> {
    type Item = IngestFileEvent;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cancel_flag.load(Ordering::SeqCst) || self.had_read_error.load(Ordering::SeqCst) {
            return None;
        }
        for (relative_path, full_path, discovered_size, mtime_ns) in self.files.by_ref() {
            if self.cancel_flag.load(Ordering::SeqCst) {
                return None;
            }
            let initial_tier = classify_content_tier(&relative_path, discovered_size, false);
            let (bytes, file_size, tier) = match initial_tier {
                ContentSupportTier::Excluded => continue,
                ContentSupportTier::Tier1PathOnly => {
                    (Vec::new(), discovered_size, ContentSupportTier::Tier1PathOnly)
                }
                ContentSupportTier::Tier2PlainText | ContentSupportTier::Tier2SourceCode => {
                    match read_bounded_text_body(&full_path, initial_tier) {
                        Ok(resolved) => resolved,
                        Err(_) => {
                            self.had_read_error.store(true, Ordering::SeqCst);
                            return None;
                        }
                    }
                }
            };
            return Some(IngestFileEvent::CreatedOrModified {
                relative_path,
                archive_entry_path: None,
                bytes,
                file_size,
                tier,
                mtime_ns,
            });
        }
        self.removed_paths
            .next()
            .map(|relative_path| IngestFileEvent::Removed { relative_path })
    }
}

fn read_bounded_text_body(
    path: &Path,
    default_tier: ContentSupportTier,
) -> io::Result<(Vec<u8>, u64, ContentSupportTier)> {
    let file = File::open(path)?;
    let mut bytes = Vec::new();
    (&file)
        .take(MAX_TEXT_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_TEXT_FILE_BYTES {
        // A file that grew since discovery retains path coverage and real metadata size without a truncated content hash.
        let actual_size = file
            .metadata()
            .map(|meta| meta.len())
            .unwrap_or(bytes.len() as u64)
            .max(bytes.len() as u64);
        return Ok((Vec::new(), actual_size, ContentSupportTier::Tier1PathOnly));
    }
    let actual_size = bytes.len() as u64;
    let effective_tier = if actual_size == 0 {
        ContentSupportTier::Tier1PathOnly
    } else {
        default_tier
    };
    Ok((bytes, actual_size, effective_tier))
}

impl SemanticSearchCoordinator {
    pub(super) fn directory_file_event_stream<'a>(
        &self,
        scope_id: &str,
        discovered_files: Vec<DiscoveredFile>,
        cancel_flag: &'a AtomicBool,
        had_read_error: &'a AtomicBool,
        had_directory_read_error: bool,
    ) -> DirectoryFileEventStream<'a> {
        let seen: HashSet<_> = discovered_files
            .iter()
            .map(|file| file.0.as_str())
            .collect();
        let mut removed_paths = Vec::new();
        if !had_directory_read_error && !cancel_flag.load(Ordering::SeqCst) {
            let guard = self.scopes.read().expect("scopes lock poisoned");
            if let Some(managed) = guard.get(scope_id) {
                removed_paths.extend(managed.index.files().filter_map(|file| {
                    (!seen.contains(file.relative_path.as_str()))
                        .then(|| file.relative_path.clone())
                }));
            }
        }
        DirectoryFileEventStream {
            files: discovered_files.into_iter(),
            removed_paths: removed_paths.into_iter(),
            cancel_flag,
            had_read_error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{SemanticIngestPhase, SemanticScopeDescriptor, SemanticScopeKind};
    use std::fs;
    use tempfile::TempDir;

    fn body_and_meta(event: IngestFileEvent) -> (String, Vec<u8>, u64, ContentSupportTier) {
        match event {
            IngestFileEvent::CreatedOrModified {
                relative_path,
                bytes,
                file_size,
                tier,
                ..
            } => (relative_path, bytes, file_size, tier),
            IngestFileEvent::Removed { .. } => panic!("expected file body"),
        }
    }

    #[test]
    fn bodies_are_opened_lazily_and_cancellation_stops_further_reads() {
        let dir = TempDir::new().expect("temporary directory");
        let first = dir.path().join("first.md");
        let second = dir.path().join("second.md");
        fs::write(&first, b"first content").expect("write first");
        let cancel = AtomicBool::new(false);
        let read_err = AtomicBool::new(false);
        let mut stream = DirectoryFileEventStream {
            files: vec![
                ("first.md".into(), first, 13, 1),
                ("second.md".into(), second.clone(), 14, 1),
            ]
            .into_iter(),
            removed_paths: vec!["removed.md".into()].into_iter(),
            cancel_flag: &cancel,
            had_read_error: &read_err,
        };
        assert_eq!(
            body_and_meta(stream.next().expect("first event")).1,
            b"first content"
        );
        // The second file did not exist when scanning/consuming the first file.
        fs::write(&second, b"second content").expect("create late body");
        assert_eq!(
            body_and_meta(stream.next().expect("late body")).1,
            b"second content"
        );
        cancel.store(true, Ordering::SeqCst);
        assert!(stream.next().is_none(), "cancel also suppresses removals");
    }

    #[test]
    fn metadata_to_read_growth_is_bounded_and_does_not_omit_later_files() {
        let dir = TempDir::new().expect("temporary directory");
        let grown = dir.path().join("grown.md");
        File::create(&grown)
            .expect("create sparse fixture")
            .set_len(MAX_TEXT_FILE_BYTES + 100)
            .expect("grow after discovery");
        let later = dir.path().join("later.md");
        fs::write(&later, b"later content").expect("write later file");
        let cancel = AtomicBool::new(false);
        let read_err = AtomicBool::new(false);
        let mut stream = DirectoryFileEventStream {
            files: vec![
                ("grown.md".into(), grown, 16, 1),
                ("later.md".into(), later, 13, 1),
            ]
            .into_iter(),
            removed_paths: Vec::new().into_iter(),
            cancel_flag: &cancel,
            had_read_error: &read_err,
        };
        let (path, bytes, file_size, tier) =
            body_and_meta(stream.next().expect("oversized file retains path"));
        assert_eq!(path, "grown.md");
        assert!(
            bytes.is_empty(),
            "grown body is path-only, never truncated content"
        );
        assert_eq!(file_size, MAX_TEXT_FILE_BYTES + 100);
        assert_eq!(tier, ContentSupportTier::Tier1PathOnly);
        let (_, later_bytes, later_size, later_tier) =
            body_and_meta(stream.next().expect("later file survives"));
        assert_eq!(later_bytes, b"later content");
        assert_eq!(later_size, 13);
        assert_eq!(later_tier, ContentSupportTier::Tier2PlainText);
        assert!(stream.next().is_none());
    }

    #[test]
    fn exact_text_limit_remains_covered() {
        let dir = TempDir::new().expect("temporary directory");
        let path = dir.path().join("limit.md");
        fs::write(&path, vec![b'a'; MAX_TEXT_FILE_BYTES as usize]).expect("write bounded fixture");
        let (bytes, size, tier) =
            read_bounded_text_body(&path, ContentSupportTier::Tier2PlainText)
                .expect("read boundary");
        assert_eq!(bytes.len(), MAX_TEXT_FILE_BYTES as usize);
        assert_eq!(size, MAX_TEXT_FILE_BYTES);
        assert_eq!(tier, ContentSupportTier::Tier2PlainText);
    }

    #[test]
    fn directory_stream_consumes_all_files_without_a_total_body_cutoff() {
        let files = TempDir::new().expect("files directory");
        let storage = TempDir::new().expect("index directory");
        let text = "database retry policy\n".repeat(2048);
        for i in 0..32 {
            fs::write(files.path().join(format!("file-{i:02}.md")), &text)
                .expect("write text file");
        }
        assert!(text.len() * 32 > MAX_TEXT_FILE_BYTES as usize);
        let engine =
            SemanticSearchCoordinator::new_for_testing_with_test_double(storage.path().into());
        engine.register_scope(SemanticScopeDescriptor {
            scope_id: "scope".into(),
            kind: SemanticScopeKind::AuthorizedDirectory,
            root_path: files.path().to_string_lossy().into(),
            display_name: "bounded".into(),
            is_authorized: true,
        });
        let progress = engine
            .index_directory_scope("scope")
            .expect("stream directory");
        assert_eq!(progress.phase, SemanticIngestPhase::Completed);
        let guard = engine.scopes.read().expect("scope lock");
        let index = &guard.get("scope").expect("scope").index;
        assert_eq!(index.files().count(), 32);
        for file in index.files() {
            assert_eq!(file.file_size as usize, text.len());
            assert!(index
                .segments()
                .iter()
                .any(|segment| segment.file_id == file.file_id && !segment.is_path_segment));
        }
    }

    #[test]
    fn oversized_and_grown_files_preserve_real_size_and_tier1_path_only_in_index() {
        let files = TempDir::new().expect("files directory");
        let storage = TempDir::new().expect("index directory");
        let oversized = files.path().join("oversized.md");
        File::create(&oversized)
            .expect("create oversized")
            .set_len(MAX_TEXT_FILE_BYTES + 512)
            .expect("set oversized len");
        let later = files.path().join("later.md");
        let later_body = b"later body token_later_777";
        fs::write(&later, later_body).expect("write later");

        let engine =
            SemanticSearchCoordinator::new_for_testing_with_test_double(storage.path().into());
        engine.register_scope(SemanticScopeDescriptor {
            scope_id: "scope".into(),
            kind: SemanticScopeKind::AuthorizedDirectory,
            root_path: files.path().to_string_lossy().into(),
            display_name: "oversized-check".into(),
            is_authorized: true,
        });
        let progress = engine
            .index_directory_scope("scope")
            .expect("index oversized directory");
        assert_eq!(progress.phase, SemanticIngestPhase::Completed);

        let guard = engine.scopes.read().expect("scope lock");
        let index = &guard.get("scope").expect("scope").index;
        let oversized_rec = index
            .files()
            .find(|f| f.relative_path == "oversized.md")
            .expect("oversized record");
        assert_eq!(oversized_rec.file_size, MAX_TEXT_FILE_BYTES + 512);
        assert_eq!(oversized_rec.tier, ContentSupportTier::Tier1PathOnly);
        assert!(index
            .segments()
            .iter()
            .filter(|s| s.file_id == oversized_rec.file_id)
            .all(|s| s.is_path_segment));

        let later_rec = index
            .files()
            .find(|f| f.relative_path == "later.md")
            .expect("later record");
        assert_eq!(later_rec.file_size, later_body.len() as u64);
        assert_eq!(later_rec.tier, ContentSupportTier::Tier2PlainText);
    }
}
