// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! In-memory directory, symlink, and hardlink safety tracker for deep archive verification.

use std::collections::HashSet;
use std::path::{Component, Path};
use ttzip_archive::fs::dir_jail::DirJail;

/// Tracks archive directory hierarchy, symlink targets, and hardlink endpoints in memory
/// during deep verification so archives with unsafe symlinks or dangling hardlinks fail
/// without materializing entries on disk.
#[derive(Default)]
pub(super) struct HardlinkTracker {
    committed_regular: HashSet<Vec<String>>,
    dir_paths: HashSet<Vec<String>>,
    symlink_paths: HashSet<Vec<String>>,
    symlinks: Vec<(Vec<String>, String)>,
    pending: Vec<(Vec<String>, Vec<String>)>,
}

impl HardlinkTracker {
    pub(super) fn parse_entry_components(raw_path: &str) -> Result<Option<Vec<String>>, String> {
        let clean_rel = raw_path.strip_suffix(";1").unwrap_or(raw_path);
        let root_check = clean_rel.trim_start_matches('/');
        if root_check.is_empty() || root_check == "." || root_check == "./" {
            if clean_rel.contains('\\') {
                return Err(
                    "Deep payload verification failed: invalid root entry path".to_string()
                );
            }
            return Ok(None);
        }
        let comps = DirJail::extract_clean_components(clean_rel).map_err(|e| {
            format!(
                "Deep payload verification failed: invalid entry path '{}' ({:?})",
                raw_path, e
            )
        })?;
        if comps.is_empty() {
            return Ok(None);
        }
        Ok(Some(comps))
    }

    fn invalidate_destination(&mut self, dst_comps: &[String]) {
        self.pending.retain(|(_, dst)| dst.as_slice() != dst_comps);
    }

    fn ensure_dir_chain(&mut self, dir_comps: &[String]) -> Result<(), String> {
        for depth in 1..=dir_comps.len() {
            let prefix = &dir_comps[..depth];
            if self.symlink_paths.contains(prefix) || self.committed_regular.contains(prefix) {
                return Err(
                    "Deep payload verification failed: ancestor path is not a directory"
                        .to_string(),
                );
            }
            self.dir_paths.insert(prefix.to_vec());
        }
        Ok(())
    }

    pub(super) fn on_directory(&mut self, entry_comps: Vec<String>) -> Result<(), String> {
        self.invalidate_destination(&entry_comps);
        self.ensure_dir_chain(&entry_comps)
    }

    pub(super) fn on_symlink(
        &mut self,
        entry_comps: Vec<String>,
        link_target: &str,
    ) -> Result<(), String> {
        let (_, parent_comps) = entry_comps
            .split_last()
            .ok_or_else(|| "Deep payload verification failed: empty symlink path".to_string())?;
        self.invalidate_destination(&entry_comps);
        self.ensure_dir_chain(parent_comps)?;
        if self.symlink_paths.contains(&entry_comps) || self.dir_paths.contains(&entry_comps) {
            return Err(
                "Deep payload verification failed: symlink collides with existing directory or symlink"
                    .to_string(),
            );
        }
        self.validate_symlink_target(&entry_comps, parent_comps, link_target)?;
        self.committed_regular.remove(&entry_comps);
        self.symlink_paths.insert(entry_comps.clone());
        self.symlinks.push((entry_comps, link_target.to_string()));
        self.verify_all_symlinks()
    }

    fn validate_raw_symlink_lexical(link_target: &str) -> Result<(), String> {
        if link_target.is_empty()
            || link_target.len() > 4096
            || link_target.contains('\0')
            || link_target.contains("://")
            || link_target.contains('\\')
            || link_target.starts_with('/')
        {
            return Err("Deep payload verification failed: unsafe symlink target".to_string());
        }
        let bytes = link_target.as_bytes();
        if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            return Err("Deep payload verification failed: unsafe symlink target".to_string());
        }
        Ok(())
    }

    fn verify_no_symlink_along(&self, comps: &[String]) -> Result<(), String> {
        for idx in 0..comps.len() {
            let prefix = &comps[..=idx];
            if self.symlink_paths.contains(prefix) {
                return Err(
                    "Deep payload verification failed: chained symlink detected".to_string()
                );
            }
            if idx + 1 < comps.len() && self.committed_regular.contains(prefix) {
                return Err(
                    "Deep payload verification failed: symlink traverses non-directory".to_string(),
                );
            }
            if !self.dir_paths.contains(prefix) && !self.committed_regular.contains(prefix) {
                return Ok(());
            }
        }
        Ok(())
    }

    fn apply_symlink_component(
        &self,
        stack: &mut Vec<String>,
        comp: Component<'_>,
    ) -> Result<(), String> {
        match comp {
            Component::CurDir => Ok(()),
            Component::ParentDir => {
                self.verify_no_symlink_along(stack)?;
                if stack.pop().is_none() {
                    return Err(
                        "Deep payload verification failed: symlink target escapes archive root"
                            .to_string(),
                    );
                }
                Ok(())
            }
            Component::Normal(seg) => {
                let s = seg.to_str().ok_or_else(|| {
                    "Deep payload verification failed: invalid symlink target".to_string()
                })?;
                if s.is_empty() || s.chars().all(|ch| ch == '.') || s.contains(':') {
                    return Err(
                        "Deep payload verification failed: invalid symlink segment".to_string()
                    );
                }
                stack.push(s.to_string());
                self.verify_no_symlink_along(stack)
            }
            Component::RootDir | Component::Prefix(_) => Err(
                "Deep payload verification failed: absolute symlink target".to_string(),
            ),
        }
    }

    fn validate_symlink_target(
        &self,
        entry_comps: &[String],
        parent_comps: &[String],
        link_target: &str,
    ) -> Result<(), String> {
        Self::validate_raw_symlink_lexical(link_target)?;
        let mut stack: Vec<String> = parent_comps.to_vec();
        self.verify_no_symlink_along(&stack)?;
        for comp in Path::new(link_target).components() {
            self.apply_symlink_component(&mut stack, comp)?;
        }
        if stack.is_empty() || entry_comps.starts_with(&stack) {
            return Err(
                "Deep payload verification failed: symlink points to root or ancestor directory"
                    .to_string(),
            );
        }
        self.verify_no_symlink_along(&stack)
    }

    fn verify_all_symlinks(&self) -> Result<(), String> {
        for (entry_comps, target) in &self.symlinks {
            let (_, parent_comps) = entry_comps.split_last().ok_or_else(|| {
                "Deep payload verification failed: empty symlink path".to_string()
            })?;
            self.validate_symlink_target(entry_comps, parent_comps, target)?;
        }
        Ok(())
    }

    fn is_non_regular(&self, comps: &[String]) -> bool {
        self.dir_paths.contains(comps) || self.symlink_paths.contains(comps)
    }

    fn commit_hardlink_endpoint(&mut self, endpoint_comps: Vec<String>) -> Result<(), String> {
        let (_, parent_comps) = endpoint_comps
            .split_last()
            .ok_or_else(|| "Deep payload verification failed: empty hardlink path".to_string())?;
        self.ensure_dir_chain(parent_comps)?;
        if self.is_non_regular(&endpoint_comps) {
            return Err(
                "Deep payload verification failed: hardlink endpoint is non-regular".to_string(),
            );
        }
        self.committed_regular.insert(endpoint_comps);
        Ok(())
    }

    pub(super) fn record_hardlink(
        &mut self,
        dst_comps: Vec<String>,
        raw_target: &str,
    ) -> Result<(), String> {
        let Some(src_comps) = Self::parse_entry_components(raw_target)? else {
            return Err("Deep payload verification failed: empty hardlink target".to_string());
        };
        let (_, dst_parent) = dst_comps
            .split_last()
            .ok_or_else(|| "Deep payload verification failed: empty hardlink path".to_string())?;
        let (_, src_parent) = src_comps
            .split_last()
            .ok_or_else(|| "Deep payload verification failed: empty hardlink target".to_string())?;
        self.invalidate_destination(&dst_comps);
        self.ensure_dir_chain(dst_parent)?;
        self.ensure_dir_chain(src_parent)?;
        if self.is_non_regular(&dst_comps) || self.is_non_regular(&src_comps) {
            return Err(
                "Deep payload verification failed: hardlink references non-regular entry"
                    .to_string(),
            );
        }
        if self.committed_regular.contains(&src_comps) {
            self.commit_hardlink_endpoint(dst_comps)?;
            return self.resolve_ready();
        }
        self.pending.push((src_comps, dst_comps));
        Ok(())
    }

    pub(super) fn on_regular_committed(
        &mut self,
        entry_comps: Vec<String>,
        raw_hardlink_target: Option<&str>,
    ) -> Result<(), String> {
        let (_, parent_comps) = entry_comps
            .split_last()
            .ok_or_else(|| "Deep payload verification failed: empty file path".to_string())?;
        self.invalidate_destination(&entry_comps);
        self.ensure_dir_chain(parent_comps)?;
        if self.is_non_regular(&entry_comps) {
            return Err(
                "Deep payload verification failed: regular entry collides with non-regular path"
                    .to_string(),
            );
        }
        self.committed_regular.insert(entry_comps.clone());
        if let Some(raw_target) = raw_hardlink_target {
            self.commit_payload_hardlink_target(&entry_comps, raw_target)?;
        }
        self.resolve_ready()
    }

    fn commit_payload_hardlink_target(
        &mut self,
        entry_comps: &[String],
        raw_target: &str,
    ) -> Result<(), String> {
        let Some(src_comps) = Self::parse_entry_components(raw_target)? else {
            return Err("Deep payload verification failed: empty hardlink target".to_string());
        };
        if src_comps != entry_comps {
            self.invalidate_destination(&src_comps);
            self.commit_hardlink_endpoint(src_comps)?;
        }
        Ok(())
    }

    fn resolve_ready(&mut self) -> Result<(), String> {
        loop {
            let mut progressed = false;
            let mut still_pending = Vec::with_capacity(self.pending.len());
            for (src_comps, dst_comps) in std::mem::take(&mut self.pending) {
                if self.committed_regular.contains(&src_comps) {
                    self.commit_hardlink_endpoint(dst_comps)?;
                    progressed = true;
                } else {
                    still_pending.push((src_comps, dst_comps));
                }
            }
            self.pending = still_pending;
            if !progressed || self.pending.is_empty() {
                break;
            }
        }
        Ok(())
    }

    pub(super) fn finalize(mut self) -> Result<(), String> {
        self.resolve_ready()?;
        loop {
            let mut progressed = false;
            let mut still_pending = Vec::with_capacity(self.pending.len());
            for (src_comps, dst_comps) in std::mem::take(&mut self.pending) {
                let resolved_endpoint = if self.committed_regular.contains(&src_comps) {
                    Some(dst_comps.clone())
                } else if self.committed_regular.contains(&dst_comps) {
                    Some(src_comps.clone())
                } else {
                    None
                };
                if let Some(target_comps) = resolved_endpoint {
                    self.commit_hardlink_endpoint(target_comps)?;
                    progressed = true;
                } else {
                    still_pending.push((src_comps, dst_comps));
                }
            }
            self.pending = still_pending;
            if !progressed || self.pending.is_empty() {
                break;
            }
        }
        if let Some((src_comps, dst_comps)) = self.pending.first() {
            return Err(format!(
                "Deep payload verification failed: unresolved hardlink '{}' -> '{}'",
                dst_comps.join("/"),
                src_comps.join("/")
            ));
        }
        self.verify_all_symlinks()
    }
}
