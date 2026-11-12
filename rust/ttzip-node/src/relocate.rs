// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use std::collections::HashMap;
use std::os::unix::io::{AsRawFd, OwnedFd};
use std::path::{Component, Path};
use ttzip_engine::fs::collision::fs_ops::{
    place_exclusive, Placement, VolCaps, RENAME_EXCL, RENAME_NOFOLLOW_ANY,
};
use ttzip_engine::fs::collision::{FsOps, ItemKind, LibcFsOps};
use ttzip_engine::fs::DirJail;
use ttzip_engine::types::{
    TTZipStatus, TTZIP_COLLISION_POLICY_KEEP_BOTH, TTZIP_COLLISION_POLICY_REPLACE,
    TTZIP_COLLISION_POLICY_SKIP,
};

const MAX_KEEP_BOTH_CANDIDATE: u32 = 10_000;
const MAX_TOTAL_KEEP_BOTH_PROBES: usize = 20_000;
const MAX_TRACKER_KEY_BYTES: usize = 4 * 1024 * 1024;

pub(crate) struct KeepBothTracker {
    next_candidate: HashMap<(Vec<String>, String), u32>,
    retained_key_bytes: usize,
    probes_used: usize,
    max_probes: usize,
}

impl KeepBothTracker {
    pub(crate) fn new() -> Self {
        Self::with_max_probes(MAX_TOTAL_KEEP_BOTH_PROBES)
    }

    fn with_max_probes(max_probes: usize) -> Self {
        Self {
            next_candidate: HashMap::new(),
            retained_key_bytes: 0,
            probes_used: 0,
            max_probes,
        }
    }

    fn start_candidate(&self, parent_comps: &[String], leaf: &str) -> u32 {
        if self.next_candidate.is_empty() {
            return 1;
        }
        self.next_candidate
            .get(&(parent_comps.to_vec(), leaf.to_string()))
            .copied()
            .unwrap_or(1)
    }

    fn record_collision(&mut self, parent_comps: &[String], leaf: &str, next_n: u32) {
        let key = (parent_comps.to_vec(), leaf.to_string());
        if let Some(slot) = self.next_candidate.get_mut(&key) {
            *slot = next_n;
            return;
        }
        let cost = 64
            + leaf.len()
            + parent_comps.iter().map(|c| 24 + c.len()).sum::<usize>();
        if self.retained_key_bytes.saturating_add(cost) <= MAX_TRACKER_KEY_BYTES {
            self.retained_key_bytes += cost;
            self.next_candidate.insert(key, next_n);
        }
    }
}

pub(crate) fn strip_leading_components(path: &str, count: u32) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let components: Vec<&str> = normalized
        .split('/')
        .filter(|c| !c.is_empty() && *c != ".")
        .collect();
    let strip = count as usize;
    if components.len() <= strip {
        return None;
    }
    Some(components[strip..].join("/"))
}

pub(crate) fn resolve_extract_collision_policy(overwrite: Option<bool>) -> u8 {
    match overwrite {
        Some(true) => TTZIP_COLLISION_POLICY_REPLACE,
        Some(false) | None => TTZIP_COLLISION_POLICY_KEEP_BOTH,
    }
}

fn status_to_io_error(st: TTZipStatus) -> std::io::Error {
    let kind = match st {
        TTZipStatus::ErrSecurityViolation => std::io::ErrorKind::PermissionDenied,
        TTZipStatus::ErrFileNotFound => std::io::ErrorKind::NotFound,
        _ => std::io::ErrorKind::Other,
    };
    std::io::Error::new(kind, format!("Extraction relocation failed: {:?}", st))
}

fn open_jailed_parent_fd(jail: &DirJail, parent_comps: &[String]) -> std::io::Result<OwnedFd> {
    jail.create_dir_all_components(parent_comps)
        .map_err(status_to_io_error)?;
    let mut cur_fd = LibcFsOps.openat_dir_nofollow(jail.root_fd(), Path::new("."))?;
    for seg in parent_comps {
        cur_fd = LibcFsOps.openat_dir_nofollow(cur_fd.as_raw_fd(), Path::new(seg))?;
    }
    Ok(cur_fd)
}

fn verify_no_symlink_along_components(jail: &DirJail, comps: &[String]) -> std::io::Result<()> {
    let mut cur_fd = LibcFsOps.openat_dir_nofollow(jail.root_fd(), Path::new("."))?;
    for (idx, seg) in comps.iter().enumerate() {
        let Some(meta) = LibcFsOps.fstatat_nofollow(cur_fd.as_raw_fd(), Path::new(seg))? else {
            return Ok(());
        };
        if meta.kind == ItemKind::Symlink {
            return Err(status_to_io_error(TTZipStatus::ErrSecurityViolation));
        }
        if idx + 1 < comps.len() {
            if meta.kind != ItemKind::Directory {
                return Err(status_to_io_error(TTZipStatus::ErrSecurityViolation));
            }
            cur_fd = LibcFsOps.openat_dir_nofollow(cur_fd.as_raw_fd(), Path::new(seg))?;
        }
    }
    Ok(())
}

fn validate_raw_symlink_target(link_target: &str) -> std::io::Result<()> {
    if link_target.is_empty()
        || link_target.len() > 4096
        || link_target.contains('\0')
        || link_target.contains("://")
        || link_target.contains('\\')
        || link_target.starts_with('/')
    {
        return Err(status_to_io_error(TTZipStatus::ErrSecurityViolation));
    }
    let bytes = link_target.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err(status_to_io_error(TTZipStatus::ErrSecurityViolation));
    }
    Ok(())
}

fn apply_symlink_target_step(
    jail: &DirJail,
    stack: &mut Vec<String>,
    comp: Component<'_>,
) -> std::io::Result<()> {
    match comp {
        Component::CurDir => Ok(()),
        Component::ParentDir => {
            verify_no_symlink_along_components(jail, stack)?;
            if stack.pop().is_none() {
                return Err(status_to_io_error(TTZipStatus::ErrSecurityViolation));
            }
            Ok(())
        }
        Component::Normal(seg) => {
            let s = seg
                .to_str()
                .ok_or_else(|| status_to_io_error(TTZipStatus::ErrSecurityViolation))?;
            if s.is_empty() || s.chars().all(|ch| ch == '.') || s.contains(':') {
                return Err(status_to_io_error(TTZipStatus::ErrSecurityViolation));
            }
            stack.push(s.to_string());
            verify_no_symlink_along_components(jail, stack)
        }
        Component::RootDir | Component::Prefix(_) => {
            Err(status_to_io_error(TTZipStatus::ErrSecurityViolation))
        }
    }
}

fn validate_stripped_symlink_target(
    jail: &DirJail,
    parent_comps: &[String],
    final_leaf: &str,
    link_target: &str,
) -> std::io::Result<()> {
    validate_raw_symlink_target(link_target)?;
    let mut stack = parent_comps.to_vec();
    verify_no_symlink_along_components(jail, &stack)?;
    for comp in Path::new(link_target).components() {
        apply_symlink_target_step(jail, &mut stack, comp)?;
    }
    let mut entry_comps = parent_comps.to_vec();
    entry_comps.push(final_leaf.to_string());
    if stack.is_empty() || entry_comps.starts_with(&stack) {
        return Err(status_to_io_error(TTZipStatus::ErrSecurityViolation));
    }
    verify_no_symlink_along_components(jail, &stack)
}

fn replace_or_skip_file_in_parent(
    src_dir_fd: i32,
    src_name: &Path,
    dst_parent_fd: i32,
    leaf_path: &Path,
    collision_policy: u8,
) -> std::io::Result<bool> {
    if collision_policy == TTZIP_COLLISION_POLICY_REPLACE {
        if let Some(existing) = LibcFsOps.fstatat_nofollow(dst_parent_fd, leaf_path)? {
            if existing.kind != ItemKind::File {
                return Err(status_to_io_error(TTZipStatus::ErrSecurityViolation));
            }
        }
        LibcFsOps.renameatx(
            src_dir_fd,
            src_name,
            dst_parent_fd,
            leaf_path,
            RENAME_NOFOLLOW_ANY,
        )?;
        return Ok(true);
    }
    if collision_policy == TTZIP_COLLISION_POLICY_SKIP {
        if LibcFsOps
            .fstatat_nofollow(dst_parent_fd, leaf_path)?
            .is_none()
        {
            let caps = VolCaps {
                rename_excl: true,
                rename_swap: false,
            };
            let placement = Placement {
                from_fd: src_dir_fd,
                from: src_name,
                to_fd: dst_parent_fd,
                to: leaf_path,
            };
            if let Err(err) = place_exclusive(&LibcFsOps, caps, placement, ItemKind::File) {
                if err.kind() != std::io::ErrorKind::AlreadyExists {
                    return Err(err);
                }
            }
        }
        let _ = LibcFsOps.unlinkat(src_dir_fd, src_name, false);
        return Ok(true);
    }
    Ok(false)
}

fn keep_both_candidate_name(leaf: &str, kind: ItemKind, n: u32) -> std::ffi::OsString {
    if n == 1 {
        std::ffi::OsString::from(leaf)
    } else {
        ttzip_engine::fs::collision::naming::candidate(std::ffi::OsStr::new(leaf), kind, n)
    }
}

fn consume_keep_both_probe(
    probes_used: &mut usize,
    max_probes: usize,
    leaf: &str,
) -> std::io::Result<()> {
    if *probes_used >= max_probes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("Exhausted collision probe budget for {}", leaf),
        ));
    }
    *probes_used += 1;
    Ok(())
}

fn place_keep_both_candidate_loop(
    parent_comps: &[String],
    dst_parent_fd: i32,
    leaf: &str,
    kind: ItemKind,
    tracker: &mut KeepBothTracker,
    mut try_place: impl FnMut(&std::ffi::OsStr, &Path) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let start_n = tracker.start_candidate(parent_comps, leaf);
    for n in start_n..=MAX_KEEP_BOTH_CANDIDATE {
        if n > 1 {
            consume_keep_both_probe(&mut tracker.probes_used, tracker.max_probes, leaf)?;
        }
        let cand_os = keep_both_candidate_name(leaf, kind, n);
        let cand_path = Path::new(&cand_os);
        if LibcFsOps
            .fstatat_nofollow(dst_parent_fd, cand_path)?
            .is_some()
        {
            if n == 1 {
                consume_keep_both_probe(&mut tracker.probes_used, tracker.max_probes, leaf)?;
            }
            tracker.record_collision(parent_comps, leaf, n + 1);
            continue;
        }
        match try_place(&cand_os, cand_path) {
            Ok(()) => {
                if n > 1 {
                    tracker.record_collision(parent_comps, leaf, n + 1);
                }
                return Ok(());
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                if n == 1 {
                    consume_keep_both_probe(&mut tracker.probes_used, tracker.max_probes, leaf)?;
                }
                tracker.record_collision(parent_comps, leaf, n + 1);
                continue;
            }
            Err(err) => return Err(err),
        }
    }
    tracker.record_collision(parent_comps, leaf, MAX_KEEP_BOTH_CANDIDATE + 1);
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        format!("Exhausted collision candidates for {}", leaf),
    ))
}

fn place_file_keep_both_in_parent(
    parent_comps: &[String],
    src_dir_fd: i32,
    src_name: &Path,
    dst_parent_fd: i32,
    leaf: &str,
    tracker: &mut KeepBothTracker,
) -> std::io::Result<()> {
    let caps = VolCaps {
        rename_excl: true,
        rename_swap: false,
    };
    place_keep_both_candidate_loop(
        parent_comps,
        dst_parent_fd,
        leaf,
        ItemKind::File,
        tracker,
        |_cand_os, cand_path| {
            let placement = Placement {
                from_fd: src_dir_fd,
                from: src_name,
                to_fd: dst_parent_fd,
                to: cand_path,
            };
            place_exclusive(&LibcFsOps, caps, placement, ItemKind::File)
        },
    )
}

fn open_staged_source_jail(src_path: &Path) -> std::io::Result<(DirJail, &Path)> {
    let src_parent = src_path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "Missing staged parent")
    })?;
    let src_name = Path::new(src_path.file_name().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "Missing staged file name")
    })?);
    let src_jail = DirJail::open(src_parent).map_err(status_to_io_error)?;
    Ok((src_jail, src_name))
}

fn move_stripped_file(
    jail: &DirJail,
    src_path: &Path,
    stripped_rel: &str,
    collision_policy: u8,
    tracker: &mut KeepBothTracker,
) -> std::io::Result<()> {
    let comps = DirJail::extract_clean_components(stripped_rel).map_err(status_to_io_error)?;
    let (leaf, parent_comps) = comps
        .split_last()
        .ok_or_else(|| status_to_io_error(TTZipStatus::ErrSecurityViolation))?;
    let dst_parent_fd = open_jailed_parent_fd(jail, parent_comps)?;
    let (src_jail, src_name) = open_staged_source_jail(src_path)?;
    if replace_or_skip_file_in_parent(
        src_jail.root_fd(),
        src_name,
        dst_parent_fd.as_raw_fd(),
        Path::new(leaf),
        collision_policy,
    )? {
        return Ok(());
    }
    place_file_keep_both_in_parent(
        parent_comps,
        src_jail.root_fd(),
        src_name,
        dst_parent_fd.as_raw_fd(),
        leaf,
        tracker,
    )
}

fn replace_or_skip_symlink_in_parent(
    jail: &DirJail,
    parent_comps: &[String],
    src_dir_fd: i32,
    src_name: &Path,
    dst_parent_fd: i32,
    leaf: &str,
    target_str: &str,
    collision_policy: u8,
) -> std::io::Result<bool> {
    let leaf_path = Path::new(leaf);
    if collision_policy == TTZIP_COLLISION_POLICY_REPLACE {
        if let Some(existing) = LibcFsOps.fstatat_nofollow(dst_parent_fd, leaf_path)? {
            if existing.kind != ItemKind::File {
                return Err(status_to_io_error(TTZipStatus::ErrSecurityViolation));
            }
        }
        validate_stripped_symlink_target(jail, parent_comps, leaf, target_str)?;
        LibcFsOps.renameatx(src_dir_fd, src_name, dst_parent_fd, leaf_path, 0)?;
        return Ok(true);
    }
    if collision_policy == TTZIP_COLLISION_POLICY_SKIP {
        if LibcFsOps
            .fstatat_nofollow(dst_parent_fd, leaf_path)?
            .is_none()
        {
            validate_stripped_symlink_target(jail, parent_comps, leaf, target_str)?;
            if let Err(err) =
                LibcFsOps.renameatx(src_dir_fd, src_name, dst_parent_fd, leaf_path, RENAME_EXCL)
            {
                if err.kind() != std::io::ErrorKind::AlreadyExists {
                    return Err(err);
                }
            }
        }
        let _ = LibcFsOps.unlinkat(src_dir_fd, src_name, false);
        return Ok(true);
    }
    Ok(false)
}

fn place_symlink_keep_both_in_parent(
    jail: &DirJail,
    parent_comps: &[String],
    src_dir_fd: i32,
    src_name: &Path,
    dst_parent_fd: i32,
    leaf: &str,
    target_str: &str,
    tracker: &mut KeepBothTracker,
) -> std::io::Result<()> {
    place_keep_both_candidate_loop(
        parent_comps,
        dst_parent_fd,
        leaf,
        ItemKind::Symlink,
        tracker,
        |cand_os, cand_path| {
            let cand_str = cand_os.to_string_lossy();
            validate_stripped_symlink_target(jail, parent_comps, &cand_str, target_str)?;
            LibcFsOps.renameatx(src_dir_fd, src_name, dst_parent_fd, cand_path, RENAME_EXCL)
        },
    )
}

fn move_stripped_symlink(
    jail: &DirJail,
    src_path: &Path,
    stripped_rel: &str,
    collision_policy: u8,
    tracker: &mut KeepBothTracker,
) -> std::io::Result<()> {
    let comps = DirJail::extract_clean_components(stripped_rel).map_err(status_to_io_error)?;
    let (leaf, parent_comps) = comps
        .split_last()
        .ok_or_else(|| status_to_io_error(TTZipStatus::ErrSecurityViolation))?;
    let dst_parent_fd = open_jailed_parent_fd(jail, parent_comps)?;
    let target = std::fs::read_link(src_path)?;
    let target_str = target.to_str().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid symlink target")
    })?;
    let (src_jail, src_name) = open_staged_source_jail(src_path)?;
    if replace_or_skip_symlink_in_parent(
        jail,
        parent_comps,
        src_jail.root_fd(),
        src_name,
        dst_parent_fd.as_raw_fd(),
        leaf,
        target_str,
        collision_policy,
    )? {
        return Ok(());
    }
    place_symlink_keep_both_in_parent(
        jail,
        parent_comps,
        src_jail.root_fd(),
        src_name,
        dst_parent_fd.as_raw_fd(),
        leaf,
        target_str,
        tracker,
    )
}

fn relocate_stripped_entry(
    stage_root: &Path,
    entry: &std::fs::DirEntry,
    jail: &DirJail,
    strip_count: u32,
    collision_policy: u8,
    tracker: &mut KeepBothTracker,
) -> std::io::Result<()> {
    let src_path = entry.path();
    let file_type = entry.file_type()?;
    let Ok(rel) = src_path.strip_prefix(stage_root) else {
        return Ok(());
    };
    let stripped = strip_leading_components(&rel.to_string_lossy(), strip_count);
    if file_type.is_dir() {
        if let Some(stripped_rel) = stripped {
            jail.create_dir_all_in_jail(&stripped_rel)
                .map_err(status_to_io_error)?;
        }
        return relocate_stripped_tree_with_state(
            stage_root, &src_path, jail, strip_count, collision_policy, tracker,
        );
    }
    let Some(stripped_rel) = stripped else {
        return Ok(());
    };
    if file_type.is_symlink() {
        return move_stripped_symlink(jail, &src_path, &stripped_rel, collision_policy, tracker);
    }
    move_stripped_file(jail, &src_path, &stripped_rel, collision_policy, tracker)
}

pub(crate) fn relocate_stripped_tree(
    stage_root: &Path,
    current_dir: &Path,
    jail: &DirJail,
    strip_count: u32,
    collision_policy: u8,
) -> std::io::Result<()> {
    let mut tracker = KeepBothTracker::new();
    relocate_stripped_tree_with_state(
        stage_root, current_dir, jail, strip_count, collision_policy, &mut tracker,
    )
}

fn relocate_stripped_tree_with_state(
    stage_root: &Path,
    current_dir: &Path,
    jail: &DirJail,
    strip_count: u32,
    collision_policy: u8,
    tracker: &mut KeepBothTracker,
) -> std::io::Result<()> {
    #[cfg(unix)]
    if let Ok(meta) = std::fs::symlink_metadata(current_dir) {
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode();
        if mode & 0o700 != 0o700 {
            let _ = std::fs::set_permissions(
                current_dir,
                std::fs::Permissions::from_mode(mode | 0o700),
            );
        }
    }
    for entry_res in std::fs::read_dir(current_dir)? {
        let entry = entry_res?;
        relocate_stripped_entry(
            stage_root, &entry, jail, strip_count, collision_policy, tracker,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_test_dir(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("{}_{}_{}", tag, std::process::id(), nanos))
    }

    #[test]
    fn test_strip_leading_components_and_relocate() {
        assert_eq!(
            strip_leading_components("root/sub/file.txt", 1),
            Some("sub/file.txt".to_string())
        );
        assert_eq!(strip_leading_components("root/file.txt", 2), None);
        assert_eq!(
            strip_leading_components("a/b/c/d.txt", 2),
            Some("c/d.txt".to_string())
        );

        let base = temp_test_dir("ttzip_node_strip_test");
        let (stage, dest) = (base.join("stage"), base.join("dest"));
        std::fs::create_dir_all(stage.join("pkg")).expect("create stage");
        std::fs::create_dir_all(&dest).expect("create dest");
        std::fs::write(dest.join("hello.txt"), b"existing").expect("write existing");

        let jail = DirJail::open(&dest).expect("open jail");
        std::fs::write(stage.join("pkg/hello.txt"), b"incoming-1").expect("write stage 1");
        relocate_stripped_tree(
            &stage,
            &stage,
            &jail,
            1,
            resolve_extract_collision_policy(None),
        )
        .expect("relocate keep_both default");
        assert_eq!(std::fs::read(dest.join("hello.txt")).unwrap(), b"existing");
        assert_eq!(
            std::fs::read(dest.join("hello 2.txt")).unwrap(),
            b"incoming-1"
        );

        std::fs::create_dir_all(dest.join("hello 3.txt")).expect("occupy candidate 3 with dir");
        std::fs::write(stage.join("pkg/hello.txt"), b"incoming-2").expect("write stage 2");
        relocate_stripped_tree(
            &stage,
            &stage,
            &jail,
            1,
            resolve_extract_collision_policy(Some(false)),
        )
        .expect("relocate overwrite=false keep_both skips occupied dir");
        assert_eq!(
            std::fs::read(dest.join("hello 4.txt")).unwrap(),
            b"incoming-2"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    #[cfg(unix)]
    fn test_strip_relocate_metadata_hardlink_and_jail() {
        use std::os::unix::fs::PermissionsExt;
        let base = temp_test_dir("ttzip_node_meta_test");
        let (stage, dest, outside) = (base.join("stage"), base.join("dest"), base.join("outside"));
        std::fs::create_dir_all(stage.join("pkg")).expect("create stage");
        std::fs::create_dir_all(&dest).expect("create dest");
        std::fs::create_dir_all(&outside).expect("create outside");
        let jail = DirJail::open(&dest).expect("open jail");

        std::fs::write(dest.join("linked.txt"), b"original-alias").expect("write linked");
        std::fs::hard_link(dest.join("linked.txt"), outside.join("alias.txt")).expect("hardlink");
        std::fs::write(stage.join("pkg/linked.txt"), b"new-content").expect("write replacement");
        relocate_stripped_tree(
            &stage,
            &stage,
            &jail,
            1,
            resolve_extract_collision_policy(Some(true)),
        )
        .expect("replace without truncating hardlink alias");
        assert_eq!(
            std::fs::read(dest.join("linked.txt")).unwrap(),
            b"new-content"
        );
        assert_eq!(
            std::fs::read(outside.join("alias.txt")).unwrap(),
            b"original-alias"
        );

        let staged_ro = stage.join("pkg/unreadable.txt");
        let expected_mtime =
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let f = std::fs::File::create(&staged_ro).expect("create unreadable");
        f.set_modified(expected_mtime).expect("set mtime");
        f.set_permissions(std::fs::Permissions::from_mode(0o000))
            .expect("chmod 000");
        drop(f);
        relocate_stripped_tree(
            &stage,
            &stage,
            &jail,
            1,
            resolve_extract_collision_policy(None),
        )
        .expect("relocate mode 000 file");
        let meta = std::fs::symlink_metadata(dest.join("unreadable.txt")).expect("stat relocated");
        assert_eq!(meta.permissions().mode() & 0o777, 0o000);
        assert_eq!(meta.modified().expect("mtime"), expected_mtime);
        let _ = std::fs::set_permissions(
            dest.join("unreadable.txt"),
            std::fs::Permissions::from_mode(0o600),
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    #[cfg(unix)]
    fn test_strip_relocate_symlinks_preserve_mtime_and_ignore_unrelated() {
        let base = temp_test_dir("ttzip_node_sym_test");
        let (stage, dest, outside) = (base.join("stage"), base.join("dest"), base.join("outside"));
        std::fs::create_dir_all(stage.join("pkg")).expect("create stage");
        std::fs::create_dir_all(&dest).expect("create dest");
        std::fs::create_dir_all(&outside).expect("create outside");
        let jail = DirJail::open(&dest).expect("open jail");

        std::os::unix::fs::symlink("/tmp", dest.join("unrelated")).expect("unrelated abs symlink");
        let staged_sym = stage.join("pkg/valid_sym");
        std::os::unix::fs::symlink("linked.txt", &staged_sym).expect("create staged sym");
        let sym_entry = ttzip_engine::fs::DeferredEntryMetadata {
            path: staged_sym.clone(),
            mode: 0o777,
            mtime_epoch_secs: 1_700_000_000,
            mtime_nanos: 0,
            is_directory: false,
            is_symlink: true,
            expected_identity: None,
        };
        ttzip_engine::fs::apply_entry_metadata_via_jail(&stage, &sym_entry, true)
            .expect("apply symlink mtime");

        relocate_stripped_tree(
            &stage,
            &stage,
            &jail,
            1,
            resolve_extract_collision_policy(None),
        )
        .expect("relocate symlink with unrelated absolute symlink present");
        let sym_meta =
            std::fs::symlink_metadata(dest.join("valid_sym")).expect("stat relocated symlink");
        assert_eq!(
            sym_meta.modified().expect("symlink mtime"),
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000)
        );

        std::os::unix::fs::symlink(&outside, dest.join("link")).expect("create symlink");
        std::fs::create_dir_all(stage.join("pkg/link")).expect("create stage link dir");
        std::fs::write(stage.join("pkg/link/escaped.txt"), b"escape").expect("write escape");
        assert!(relocate_stripped_tree(
            &stage,
            &stage,
            &jail,
            1,
            resolve_extract_collision_policy(Some(true)),
        )
        .is_err());
        assert!(!outside.join("escaped.txt").exists());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn test_strip_keep_both_tracks_suffixes_and_probe_budget() {
        let base = temp_test_dir("ttzip_node_keepboth_quota");
        let (stage, dest) = (base.join("stage"), base.join("dest"));
        std::fs::create_dir_all(&stage).expect("create stage");
        std::fs::create_dir_all(&dest).expect("create dest");
        let jail = DirJail::open(&dest).expect("open jail");

        for i in 1..=32 {
            let dir = stage.join(format!("d{}", i));
            std::fs::create_dir_all(&dir).expect("create dN");
            std::fs::write(dir.join("file.txt"), format!("v{}", i)).expect("write dN/file.txt");
            std::fs::write(dir.join(format!("unique_{}.txt", i)), b"u").expect("write unique");
        }
        let mut tracker = KeepBothTracker::new();
        relocate_stripped_tree_with_state(
            &stage,
            &stage,
            &jail,
            1,
            TTZIP_COLLISION_POLICY_KEEP_BOTH,
            &mut tracker,
        )
        .expect("relocate 32 colliding files in linear probes");
        assert_eq!(tracker.probes_used, 32);
        assert_eq!(tracker.next_candidate.len(), 1);
        assert_eq!(
            tracker.next_candidate.get(&(Vec::new(), "file.txt".to_string())),
            Some(&33)
        );
        assert!(dest.join("file.txt").exists());
        assert!(dest.join("file 32.txt").exists());
        assert!(dest.join("unique_32.txt").exists());

        std::fs::create_dir_all(stage.join("d33")).expect("create d33");
        std::fs::write(stage.join("d33/file.txt"), b"budget-hit").expect("write d33/file.txt");
        let mut tight_tracker = KeepBothTracker::with_max_probes(5);
        let err = relocate_stripped_tree_with_state(
            &stage,
            &stage,
            &jail,
            1,
            TTZIP_COLLISION_POLICY_KEEP_BOTH,
            &mut tight_tracker,
        )
        .expect_err("exceeding probe budget must fail");
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(tight_tracker.probes_used, 5);

        let _ = std::fs::remove_dir_all(&base);
    }
}
