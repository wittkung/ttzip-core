// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Safe extraction pipeline with ZipSlip path traversal defense and two-stage
//! bottom-up POSIX permission and timestamp restoration.

pub use super::dir_jail::DirJail;
use crate::types::TTZipStatus;
use std::borrow::Cow;
use std::fs::{self, File};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};
use unicode_normalization::{is_nfc_quick, IsNormalized, UnicodeNormalization};

/// Maps Windows backslashes `\` to POSIX forward slashes `/` in-place within a mutable byte slice.
///
/// Because both `\` (`0x5C`) and `/` (`0x2F`) are 1-byte ASCII characters, this transformation
/// strictly preserves length and UTF-8 validity with zero heap allocations.
#[inline]
pub fn normalize_slashes_in_place(bytes: &mut [u8]) {
    for b in bytes.iter_mut() {
        if *b == b'\\' {
            *b = b'/';
        }
    }
}

/// Maps Windows backslashes `\` to POSIX forward slashes `/` in-place within a mutable string buffer.
#[inline]
pub fn normalize_slashes_in_place_str(s: &mut str) {
    // SAFETY: Replacing ASCII byte 0x5C with ASCII byte 0x2F strictly preserves UTF-8 invariants.
    unsafe {
        normalize_slashes_in_place(s.as_bytes_mut());
    }
}

/// Normalizes Windows backslashes `\` to POSIX forward slashes `/`.
///
/// Returns `Cow::Borrowed` with zero allocations when no backslashes are present,
/// or an in-place mutated `Cow::Owned` string when backslashes are encountered.
#[inline]
pub fn normalize_slashes(path: &str) -> Cow<'_, str> {
    if !path.as_bytes().contains(&b'\\') {
        Cow::Borrowed(path)
    } else {
        let mut owned = path.to_string();
        normalize_slashes_in_place_str(&mut owned);
        Cow::Owned(owned)
    }
}

/// Normalizes a path string to Unicode Canonical Composition (NFC) form.
///
/// Employs a zero-allocation fast-path for pure ASCII or pre-normalized inputs.
#[inline]
pub fn normalize_to_nfc(path: &str) -> Cow<'_, str> {
    if path.is_ascii() || is_nfc_quick(path.chars()) == IsNormalized::Yes {
        Cow::Borrowed(path)
    } else {
        Cow::Owned(path.nfc().collect::<String>())
    }
}

/// Strictly validates that no intermediate ancestor directory between dest_dir and target is a symlink.
pub fn validate_no_intermediate_symlinks(dest_dir: &Path, target: &Path) -> Result<(), TTZipStatus> {
    let mut current = dest_dir.to_path_buf();
    let relative = match target.strip_prefix(dest_dir) {
        Ok(rel) => rel,
        Err(_) => return Err(TTZipStatus::ErrSecurityViolation),
    };

    let mut components = relative.components().peekable();
    while let Some(comp) = components.next() {
        // If this is the leaf component, stop intermediate check
        if components.peek().is_none() {
            break;
        }
        current.push(comp);
        if let Ok(meta) = fs::symlink_metadata(&current) {
            if meta.file_type().is_symlink() {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
        }
    }
    Ok(())
}

/// Checks if a path component is a Windows reserved device name, either as a standalone
/// name (e.g. `CON`, `nul`) or with an extension (e.g. `CON.txt`, `aux.tar.gz`).
#[inline]
pub fn is_windows_reserved_device(comp: &str) -> bool {
    crate::security::path_sanitizer::is_windows_reserved_device_name(comp)
}

/// Sanitizes a relative entry path and strictly validates that it does not escape `dest_dir`.
///
/// Defends against:
/// 1. Path traversal (`..`, `../..`, `/..`)
/// 2. Absolute root paths (`/etc/passwd`), UNC paths (`//share`, `\\share`), and drive letters (`C:`)
/// 3. Backslashes (`\`) and Windows illegal characters (`< > : " | ? *`)
/// 4. Embedded null bytes and ASCII control characters (`0x00..=0x1F`)
/// 5. Windows reserved device names (`CON`, `PRN`, `AUX`, `NUL`, `COM0-9`, `LPT0-9`, `CLOCK$`, `CONIN$`, `CONOUT$`)
/// 6. NTFS Alternate Data Streams (`:`)
/// 7. Win32 normalization trailing space/dot truncation (`foo `, `foo.`) and leading space (` foo`)
/// 8. Path total length upper bound (<= 4096 bytes)
/// 9. Unicode normalization desynchronization via canonical NFC formatting
pub fn sanitize_and_validate_path(dest_dir: &Path, raw_entry_path: &str) -> Result<PathBuf, TTZipStatus> {
    let raw_bytes = raw_entry_path.as_bytes();
    let len = raw_bytes.len();

    // 1. Length bounds (1..=4096 bytes)
    if len == 0 || len > 4096 {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    // 2. Reject root slash, UNC paths (// or \\), or scheme delimiters (://)
    if raw_bytes[0] == b'/' || raw_entry_path.starts_with("//") || raw_entry_path.contains("://") {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    // 3. Reject Windows drive letter prefix (e.g. "C:", "D:")
    if len >= 2 && raw_bytes[0].is_ascii_alphabetic() && raw_bytes[1] == b':' {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    // 4. Pure byte scan: reject control characters (0x00..=0x1F), backslashes ('\'),
    // and Windows illegal characters (< > : " | ? *)
    for &b in raw_bytes {
        if b <= 0x1F || b == b'\\' || matches!(b, b'<' | b'>' | b':' | b'"' | b'|' | b'?' | b'*') {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
    }

    // 5. Canonical Unicode NFC normalization
    let nfc_normalized = normalize_to_nfc(raw_entry_path);
    let normalized_path = nfc_normalized.as_ref();
    if normalized_path.len() > 4096 {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    // 6. Component-level structural and Win32 normalization validation
    let mut target = dest_dir.to_path_buf();
    let mut has_normal_components = false;

    for comp in Path::new(normalized_path).components() {
        match comp {
            Component::Normal(c) => {
                let s = match c.to_str() {
                    Some(s) => s,
                    None => return Err(TTZipStatus::ErrSecurityViolation),
                };

                // Win32 normalization trailing truncation & leading space defense:
                // Components must not start with space, and must not end with space or dot (except AppleDouble root ._.).
                if s.starts_with(' ') || s.ends_with(' ') || (s.ends_with('.') && s != "._.") {
                    return Err(TTZipStatus::ErrSecurityViolation);
                }

                // Reject multi-dot components ("..", "...", etc.)
                if s.chars().all(|ch| ch == '.') {
                    return Err(TTZipStatus::ErrSecurityViolation);
                }

                // Defense against NTFS Alternate Data Streams (ADS) and Windows reserved devices
                if s.contains(':') || is_windows_reserved_device(s) {
                    return Err(TTZipStatus::ErrSecurityViolation);
                }

                target.push(s);
                has_normal_components = true;
            }
            Component::CurDir => {
                // Standalone '.' relative component is safe to skip
            }
            Component::RootDir | Component::Prefix(_) | Component::ParentDir => {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
        }
    }

    if !has_normal_components || !target.starts_with(dest_dir) {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    Ok(target)
}

/// Validates and normalizes an archive entry relative path for ZIP archive creation.
///
/// Strictly conforms to PKWARE APPNOTE.TXT Section 4.4.17.1:
/// - Maps Windows backslashes `\` to POSIX forward slashes `/`.
/// - Prohibits leading slashes, drive letters, and parent directory traversal (`..`).
/// - Prohibits embedded null bytes (`\0`) and control characters (`0x00..=0x1F`).
/// - Prohibits NTFS Alternate Data Streams (`:`).
/// - Enforces maximum length bounds (<= 4096 bytes).
/// - Appends trailing `/` for directories if absent.
pub fn validate_and_normalize_zip_rel_path(raw_rel_path: &str, is_directory: bool) -> Result<String, TTZipStatus> {
    if raw_rel_path.is_empty() || raw_rel_path.len() > 4096 || raw_rel_path.contains('\0') {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    // In-place slash normalization: convert Windows '\' to POSIX '/'
    let mut normalized = raw_rel_path.to_string();
    normalize_slashes_in_place_str(&mut normalized);

    // Reject absolute paths, UNC prefixes, or scheme delimiters
    if normalized.starts_with('/') || normalized.starts_with("//") || normalized.contains("://") {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    // Reject Windows drive letters (e.g. "C:")
    let bytes = normalized.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    // Reject NTFS alternate data streams or control characters
    for &b in bytes {
        if b <= 0x1F || b == b':' {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
    }

    // Component-level structural and traversal validation
    let mut has_normal = false;
    for comp in Path::new(&normalized).components() {
        match comp {
            Component::Normal(s) => {
                let s_str = match s.to_str() {
                    Some(val) => val,
                    None => return Err(TTZipStatus::ErrSecurityViolation),
                };
                if s_str.chars().all(|ch| ch == '.') || is_windows_reserved_device(s_str) {
                    return Err(TTZipStatus::ErrSecurityViolation);
                }
                has_normal = true;
            }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) | Component::ParentDir => {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
        }
    }

    if !has_normal {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    if is_directory && !normalized.ends_with('/') {
        normalized.push('/');
    }

    Ok(normalized)
}

/// Metadata record for deferred application after archive extraction completes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeferredEntryMetadata {
    pub path: PathBuf,
    pub mode: u32,
    pub mtime_epoch_secs: i64,
    pub mtime_nanos: u32,
    pub is_directory: bool,
    pub is_symlink: bool,
    pub expected_identity: Option<(u64, u64)>,
}

impl DeferredEntryMetadata {
    /// Depth of the directory tree for bottom-up sorting.
    pub fn depth(&self) -> usize {
        self.path.components().count()
    }
}

/// Engine managing two-stage safe extraction and bottom-up metadata application.
#[derive(Debug, Default)]
pub struct SafeExtractEngine {
    pub dest_root: PathBuf,
    deferred_entries: Vec<DeferredEntryMetadata>,
    preexisting_dirs: std::collections::HashSet<PathBuf>,
}

impl SafeExtractEngine {
    pub fn new() -> Self {
        Self {
            dest_root: PathBuf::new(),
            deferred_entries: Vec::new(),
            preexisting_dirs: std::collections::HashSet::new(),
        }
    }

    pub fn for_root(dest_root: impl Into<PathBuf>) -> Self {
        let root = dest_root.into();
        let preexisting_dirs = snapshot_existing_dirs(&root);
        Self {
            dest_root: root,
            deferred_entries: Vec::new(),
            preexisting_dirs,
        }
    }

    pub fn set_dest_root(&mut self, dest_root: impl Into<PathBuf>) {
        self.dest_root = dest_root.into();
        self.preexisting_dirs = snapshot_existing_dirs(&self.dest_root);
    }

    /// Registers an entry for deferred metadata application.
    pub fn register_entry(
        &mut self,
        path: PathBuf,
        mode: u32,
        mtime_epoch_secs: i64,
        mtime_nanos: u32,
        is_directory: bool,
    ) {
        self.register_entry_typed(path, mode, mtime_epoch_secs, mtime_nanos, is_directory, false);
    }

    /// Registers an entry with explicit directory and symlink discrimination.
    pub fn register_entry_typed(
        &mut self,
        path: PathBuf,
        mode: u32,
        mtime_epoch_secs: i64,
        mtime_nanos: u32,
        is_directory: bool,
        is_symlink: bool,
    ) {
        self.deferred_entries.push(DeferredEntryMetadata {
            path,
            mode,
            mtime_epoch_secs,
            mtime_nanos,
            is_directory,
            is_symlink,
            expected_identity: None,
        });
    }

    /// Applies all deferred metadata with permissions preserved.
    pub fn apply_all(&mut self) -> Result<(), TTZipStatus> {
        self.apply_deferred_metadata(true)
    }

    /// Creates directory hierarchy with temporary restricted `0700` POSIX permissions.
    pub fn create_dir_all_secure(&mut self, path: &Path, mode: u32, mtime_secs: i64) -> std::io::Result<()> {
        if !self.dest_root.as_os_str().is_empty() && path.starts_with(&self.dest_root) {
            create_dir_in_dir_nofollow(&self.dest_root, path).map_err(std::io::Error::from)?;
        } else {
            let root = path.parent().unwrap_or(path);
            validate_no_intermediate_symlinks(root, path)
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Symlink in parent path"))?;

            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            builder.mode(0o700);
            builder.create(path)?;
        }

        self.deferred_entries.push(DeferredEntryMetadata {
            path: path.to_path_buf(),
            mode,
            mtime_epoch_secs: mtime_secs,
            mtime_nanos: 0,
            is_directory: true,
            is_symlink: false,
            expected_identity: None,
        });

        Ok(())
    }

    /// Securely creates and opens a destination file with `O_NOFOLLOW | O_CLOEXEC` flag to prevent symlink hijack.
    pub fn create_file_secure(
        &mut self,
        path: &Path,
        mode: u32,
        mtime_secs: i64,
        collision_policy: bool,
    ) -> std::io::Result<File> {
        let file = if !self.dest_root.as_os_str().is_empty() && path.starts_with(&self.dest_root) {
            let jail = DirJail::open(&self.dest_root).map_err(std::io::Error::from)?;
            let comps = jail.resolve_path_components(path).map_err(std::io::Error::from)?;
            jail.open_file_components(&comps, 0o600, collision_policy)
                .map_err(std::io::Error::from)?
        } else {
            let root = path.parent().unwrap_or(path);
            validate_no_intermediate_symlinks(root, path)
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Symlink in parent path"))?;

            if let Ok(meta) = fs::symlink_metadata(path) {
                if meta.file_type().is_symlink() || meta.file_type().is_dir() {
                    return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Target is a symlink or directory"));
                }
            }

            let mut options = fs::OpenOptions::new();
            options.write(true).create(true);
            if collision_policy {
                options.truncate(true);
            } else {
                options.create_new(true);
            }
            options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
            options.mode(0o600);
            options.open(path)?
        };

        self.deferred_entries.push(DeferredEntryMetadata {
            path: path.to_path_buf(),
            mode,
            mtime_epoch_secs: mtime_secs,
            mtime_nanos: 0,
            is_directory: false,
            is_symlink: false,
            expected_identity: None,
        });

        Ok(file)
    }

    /// Default post-extraction POSIX permission for implicit parent directories
    /// (ancestor directories created on demand when an archive omits explicit directory headers)
    /// and explicit directory entries whose header omits POSIX mode bits (`mode == 0`).
    pub const DEFAULT_IMPLICIT_DIR_MODE: u32 = 0o755;

    /// Default post-extraction POSIX permission for newly extracted regular files when
    /// `preserve_permissions == false` (owner-writable, non-executable, non-world-writable).
    pub const DEFAULT_REGULAR_FILE_MODE: u32 = 0o644;

    /// Stage 2: Post-extraction deferred metadata application in Bottom-Up order.
    ///
    /// Directories are sorted deepest-first (longest path / highest component count)
    /// to prevent changing parent permissions (e.g. to read-only 0555) before child
    /// entries have their metadata and timestamps applied.
    pub fn apply_deferred_metadata(&mut self, preserve_permissions: bool) -> Result<(), TTZipStatus> {
        let drained: Vec<DeferredEntryMetadata> = self.deferred_entries.drain(..).collect();
        let (mut files, mut dirs) = super::staged_metadata::dedup_last_entry_wins(drained);
        let mut implicit_dirs = self.collect_implicit_parent_dirs(&files, &dirs);
        let creation_mode_mask =
            super::staged_metadata::probe_creation_mode_mask_for_path(&self.dest_root);

        if !self.dest_root.as_os_str().is_empty() {
            super::staged_metadata::populate_staged_identities(&self.dest_root, &mut files)
                .map_err(|e| e.status)?;
            super::staged_metadata::populate_staged_identities(&self.dest_root, &mut dirs)
                .map_err(|e| e.status)?;
            super::staged_metadata::populate_staged_identities(&self.dest_root, &mut implicit_dirs)
                .map_err(|e| e.status)?;
        }

        if super::staged_metadata::is_transactional_staging_payload(&self.dest_root) {
            super::staged_metadata::store_staged_metadata_plan(
                super::staged_metadata::StagedMetadataPlan {
                    staging_payload_root: self.dest_root.clone(),
                    preserve_permissions,
                    creation_mode_mask,
                    files,
                    explicit_dirs: dirs,
                    implicit_dirs,
                },
            );
            return Ok(());
        }

        dirs.extend(implicit_dirs);

        // 1. Apply file metadata first via DirJail FD traversal
        for file in files {
            super::staged_metadata::apply_entry_metadata_via_jail_with_mask(
                &self.dest_root,
                &file,
                preserve_permissions,
                creation_mode_mask,
            )
            .map_err(|e| e.status)?;
        }

        // 2. Sort directories Bottom-Up (descending depth, deepest child first)
        let mut decorated_dirs: Vec<(usize, DeferredEntryMetadata)> = dirs
            .into_iter()
            .map(|d| (d.depth(), d))
            .collect();
        decorated_dirs.sort_by(|(depth_a, dir_a), (depth_b, dir_b)| {
            depth_b.cmp(depth_a).then_with(|| dir_b.path.cmp(&dir_a.path))
        });

        // 3. Apply directory metadata in bottom-up sequence via DirJail FD traversal
        for (_, dir) in decorated_dirs {
            super::staged_metadata::apply_entry_metadata_via_jail_with_mask(
                &self.dest_root,
                &dir,
                preserve_permissions,
                creation_mode_mask,
            )
            .map_err(|e| e.status)?;
            self.preexisting_dirs.insert(dir.path);
        }

        Ok(())
    }

    fn collect_implicit_parent_dirs(
        &self,
        files: &[DeferredEntryMetadata],
        explicit_dirs: &[DeferredEntryMetadata],
    ) -> Vec<DeferredEntryMetadata> {
        if self.dest_root.as_os_str().is_empty() {
            return Vec::new();
        }
        let explicit_paths: std::collections::HashSet<&Path> =
            explicit_dirs.iter().map(|d| d.path.as_path()).collect();
        let mut seen_implicit = std::collections::HashSet::new();
        let mut implicit_dirs = Vec::new();

        for entry in files.iter().chain(explicit_dirs.iter()) {
            let mut cur = entry.path.parent();
            while let Some(parent) = cur {
                if parent == self.dest_root || !parent.starts_with(&self.dest_root) {
                    break;
                }
                if !explicit_paths.contains(parent)
                    && !self.preexisting_dirs.contains(parent)
                    && seen_implicit.insert(parent.to_path_buf())
                {
                    implicit_dirs.push(DeferredEntryMetadata {
                        path: parent.to_path_buf(),
                        mode: 0,
                        mtime_epoch_secs: 0,
                        mtime_nanos: 0,
                        is_directory: true,
                        is_symlink: false,
                        expected_identity: None,
                    });
                }
                cur = parent.parent();
            }
        }

        implicit_dirs
    }
}

/// Securely creates/opens an extracted file with `O_NOFOLLOW | O_CLOEXEC`,
/// rejecting any pre-existing symlink or directory at the destination path.
pub fn open_extracted_file_strictly_nofollow(path: &Path) -> std::io::Result<File> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() || meta.file_type().is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Target is a symlink or directory",
            ));
        }
    }

    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    options.mode(0o600);
    options.open(path)
}

/// Securely creates/opens an extracted file within `dest_dir` via step-by-step `DirJail` resolution.
pub fn open_extracted_file_in_dir_nofollow(dest_dir: &Path, safe_path: &Path) -> Result<File, TTZipStatus> {
    let jail = DirJail::open(dest_dir)?;
    let comps = jail.resolve_path_components(safe_path)?;
    jail.open_file_components(&comps, 0o600, true)
}

/// Securely creates a directory hierarchy within `dest_dir` via step-by-step `DirJail` resolution.
pub fn create_dir_in_dir_nofollow(dest_dir: &Path, safe_path: &Path) -> Result<(), TTZipStatus> {
    let jail = DirJail::open(dest_dir)?;
    if safe_path == dest_dir {
        return Ok(());
    }
    let comps = jail.resolve_path_components(safe_path)?;
    jail.create_dir_all_components(&comps)
}

/// Securely validates and creates a symlink within `dest_dir` via `DirJail` (`symlinkat` + chained symlink defense).
pub fn create_symlink_in_dir_nofollow(
    dest_dir: &Path,
    safe_path: &Path,
    link_target: &str,
) -> Result<(), TTZipStatus> {
    let jail = DirJail::open(dest_dir)?;
    let comps = jail.resolve_path_components(safe_path)?;
    jail.create_symlink_components(&comps, link_target)
}

/// Securely validates and creates a hardlink within `dest_dir` via `DirJail` (`linkat` without following symlinks).
pub fn create_hardlink_in_dir_nofollow(
    dest_dir: &Path,
    dst_safe_path: &Path,
    src_safe_path: &Path,
) -> Result<(), TTZipStatus> {
    let jail = DirJail::open(dest_dir)?;
    let dst_comps = jail.resolve_path_components(dst_safe_path)?;
    let src_comps = jail.resolve_path_components(src_safe_path)?;
    jail.create_hardlink_components(&dst_comps, &src_comps)
}

fn snapshot_existing_dirs(root: &Path) -> std::collections::HashSet<PathBuf> {
    let mut set = std::collections::HashSet::new();
    if root.as_os_str().is_empty() {
        return set;
    }
    let Ok(meta) = fs::symlink_metadata(root) else {
        return set;
    };
    if !meta.file_type().is_dir() || meta.file_type().is_symlink() {
        return set;
    }

    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if let Ok(em) = fs::symlink_metadata(&path) {
                if em.file_type().is_dir() && !em.file_type().is_symlink() {
                    set.insert(path.clone());
                    stack.push(path);
                }
            }
        }
    }
    set
}

