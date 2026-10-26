// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Entry selection for partial and single-entry extraction (feature 229, US2).
//!
//! A single `EntrySelection` value decides both *which* archive entries are
//! extracted and *where* each one lands below the staging root. Preflight
//! conflict detection and staging consume the same value, so the paths probed
//! at the destination are exactly the paths that will later be committed.

use std::collections::HashSet;

/// Strips every leading `./` and `/` from an archive entry path.
#[inline]
pub fn normalize_entry_path(path: &str) -> &str {
    let mut p = path;
    loop {
        if let Some(rest) = p.strip_prefix("./") {
            p = rest;
        } else if let Some(rest) = p.strip_prefix('/') {
            p = rest;
        } else {
            return p;
        }
    }
}

/// Lookup key of an entry path: normalized and without trailing `/`.
#[inline]
fn selection_key(path: &str) -> &str {
    normalize_entry_path(path).trim_end_matches('/')
}

/// Parent directory of a selection key (`""` for top-level names).
#[inline]
fn parent_dir(key: &str) -> &str {
    key.rsplit_once('/').map_or("", |(parent, _)| parent)
}

/// Longest whole-component prefix shared by two directory paths, borrowed from `a`.
fn shared_dir_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let mut end = 0usize;
    for (ca, cb) in a.split('/').zip(b.split('/')) {
        if ca != cb {
            break;
        }
        end = if end == 0 { ca.len() } else { end + 1 + ca.len() };
    }
    &a[..end]
}

/// Deepest directory that is a strict ancestor of every selected key.
fn common_parent(targets: &HashSet<String>) -> Option<String> {
    let mut parents = targets.iter().map(|t| parent_dir(t));
    let mut common = parents.next()?;
    for parent in parents {
        common = shared_dir_prefix(common, parent);
        if common.is_empty() {
            return None;
        }
    }
    (!common.is_empty()).then(|| common.to_owned())
}

/// Set of selected archive entries plus the destination path remapping rule.
///
/// Selecting a directory selects all of its descendants. With
/// `root_at_common_parent`, the deepest directory shared by all selected paths
/// is removed from every output path: selecting `docs/report.pdf` yields
/// `report.pdf`; selecting `docs/img/` yields `img/...`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntrySelection {
    targets: HashSet<String>,
    strip_prefix: Option<String>,
}

impl EntrySelection {
    pub fn new<I, S>(targets: I, root_at_common_parent: bool) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let targets: HashSet<String> = targets
            .into_iter()
            .map(|t| selection_key(t.as_ref()).to_owned())
            .filter(|t| !t.is_empty())
            .collect();
        let strip_prefix = if root_at_common_parent {
            common_parent(&targets)
        } else {
            None
        };
        Self { targets, strip_prefix }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Returns true when `entry_path` is selected or lies below a selected directory.
    /// Performs at most `depth + 1` hash lookups and no allocation.
    pub fn contains(&self, entry_path: &str) -> bool {
        let key = selection_key(entry_path);
        if key.is_empty() {
            return false;
        }
        self.targets.contains(key)
            || key
                .match_indices('/')
                .any(|(i, _)| self.targets.contains(&key[..i]))
    }

    /// Destination path relative to the staging root, or `None` when the entry
    /// is not selected. Trailing `/` of directory entries is preserved.
    pub fn output_path<'a>(&self, entry_path: &'a str) -> Option<&'a str> {
        if !self.contains(entry_path) {
            return None;
        }
        let clean = normalize_entry_path(entry_path);
        // The prefix is a strict ancestor of every selected key, so for a selected
        // entry the strip always succeeds; `unwrap_or` only guards foreign input.
        Some(match &self.strip_prefix {
            Some(prefix) => clean
                .strip_prefix(prefix.as_str())
                .and_then(|rest| rest.strip_prefix('/'))
                .unwrap_or(clean),
            None => clean,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_dot_slash_and_root() {
        assert_eq!(normalize_entry_path("./a/b"), "a/b");
        assert_eq!(normalize_entry_path("/./a"), "a");
        assert_eq!(normalize_entry_path("a/./b"), "a/./b");
        assert_eq!(normalize_entry_path("./"), "");
    }

    #[test]
    fn exact_and_descendant_matching() {
        let sel = EntrySelection::new(["docs/img/", "notes.txt"], false);
        assert!(sel.contains("docs/img/"));
        assert!(sel.contains("docs/img"));
        assert!(sel.contains("./docs/img/a.png"));
        assert!(sel.contains("docs/img/deep/b.png"));
        assert!(sel.contains("notes.txt"));
        assert!(!sel.contains("docs/"));
        assert!(!sel.contains("docs/imgx/a.png"));
        assert!(!sel.contains("notes.txt.bak"));
        assert!(!sel.contains("./"));
    }

    #[test]
    fn output_paths_without_remapping_are_normalized() {
        let sel = EntrySelection::new(["docs/report.pdf"], false);
        assert_eq!(sel.output_path("./docs/report.pdf"), Some("docs/report.pdf"));
        assert_eq!(sel.output_path("docs/other.pdf"), None);
    }

    #[test]
    fn common_parent_remapping_matches_data_model() {
        let file = EntrySelection::new(["docs/report.pdf"], true);
        assert_eq!(file.output_path("docs/report.pdf"), Some("report.pdf"));

        let dir = EntrySelection::new(["docs/img/"], true);
        assert_eq!(dir.output_path("docs/img/"), Some("img/"));
        assert_eq!(dir.output_path("docs/img/a.png"), Some("img/a.png"));

        let siblings = EntrySelection::new(["a/b/c/1.txt", "a/b/c/2.txt"], true);
        assert_eq!(siblings.output_path("a/b/c/1.txt"), Some("1.txt"));

        let divergent = EntrySelection::new(["a/b/1.txt", "a/c/2.txt"], true);
        assert_eq!(divergent.output_path("a/b/1.txt"), Some("b/1.txt"));
        assert_eq!(divergent.output_path("a/c/2.txt"), Some("c/2.txt"));
    }

    #[test]
    fn no_common_parent_keeps_full_paths() {
        let top = EntrySelection::new(["report.pdf"], true);
        assert_eq!(top.output_path("report.pdf"), Some("report.pdf"));

        let disjoint = EntrySelection::new(["docs/1.txt", "notes.txt"], true);
        assert_eq!(disjoint.output_path("docs/1.txt"), Some("docs/1.txt"));

        let partial_name = EntrySelection::new(["ab/x.txt", "abc/y.txt"], true);
        assert_eq!(partial_name.output_path("ab/x.txt"), Some("ab/x.txt"));
    }

    #[test]
    fn empty_and_blank_targets_select_nothing() {
        let none: [&str; 0] = [];
        assert!(EntrySelection::new(none, true).is_empty());
        let blank = EntrySelection::new(["./", "/"], true);
        assert!(blank.is_empty());
        assert_eq!(blank.output_path("a.txt"), None);
    }
}
