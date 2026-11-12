// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Mozilla UniFFI Concurrent Directory Scanner & POSIX Path Normalization Layer.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use super::defensive;
use super::types::{DiskItemSummary, PathSuggestionItem, TTZipError, UniFFIParentAndPrefix};

static LAST_SCAN_IO_ERROR_COUNT: AtomicU64 = AtomicU64::new(0);

/// Returns the number of per-entry or subdirectory I/O errors encountered during the most recent `scan_directory` call.
#[inline]
pub fn last_scan_io_error_count() -> u64 {
    LAST_SCAN_IO_ERROR_COUNT.load(Ordering::Relaxed)
}

#[cfg(target_os = "macos")]
const SCAN_BUFFER_SIZE: usize = 256 * 1024; // 256 KB thread-local aligned buffer

#[cfg(target_os = "macos")]
const ATTR_CMN_ERROR: libc::attrgroup_t = 0x20000000;

#[cfg(target_os = "macos")]
#[repr(align(8))]
struct AlignedScanBuffer([u8; SCAN_BUFFER_SIZE]);

#[cfg(target_os = "macos")]
thread_local! {
    static SCAN_BUFFER: std::cell::RefCell<Box<AlignedScanBuffer>> =
        std::cell::RefCell::new(Box::new(AlignedScanBuffer([0u8; SCAN_BUFFER_SIZE])));
}

#[cfg(target_os = "macos")]
struct DirFdGuard(libc::c_int);

#[cfg(target_os = "macos")]
impl DirFdGuard {
    #[inline]
    fn new(fd: libc::c_int) -> Self {
        Self(fd)
    }

    #[inline]
    fn raw(&self) -> libc::c_int {
        self.0
    }
}

#[cfg(target_os = "macos")]
impl Drop for DirFdGuard {
    fn drop(&mut self) {
        if self.0 >= 0 {
            unsafe {
                libc::close(self.0);
            }
        }
    }
}

/// Scans a directory and returns lightweight item summaries along with the count of non-fatal I/O errors.
pub fn scan_directory_with_error_count(path: String, max_depth: u32) -> Result<(Vec<DiskItemSummary>, u64), TTZipError> {
    defensive::apply_vfs_dataless_materialization_policy();
    if !Path::new(&path).exists() {
        return Err(TTZipError::FileNotFound { path });
    }

    let mut io_error_count = 0u64;
    #[cfg(target_os = "macos")]
    let items = match scan_directory_darwin(&path, max_depth, &mut io_error_count) {
        Ok(items) => items,
        Err(e) => {
            log::debug!("getattrlistbulk scan failed: {e:?}, falling back to read_dir");
            io_error_count = 0;
            scan_directory_fallback(&path, max_depth, &mut io_error_count)?
        }
    };
    #[cfg(not(target_os = "macos"))]
    let items = scan_directory_fallback(&path, max_depth, &mut io_error_count)?;

    LAST_SCAN_IO_ERROR_COUNT.store(io_error_count, Ordering::Relaxed);
    if io_error_count > 0 {
        log::debug!("scan_directory completed with {io_error_count} non-fatal entry/subdir I/O error(s)");
    }
    Ok((items, io_error_count))
}

/// Scans a directory and returns lightweight item summaries.
// On macOS, utilizes `getattrlistbulk` with a thread-local 256KB aligned buffer and relative
// `openat` descriptor traversal to completely avoid `PATH_MAX` limitations and string heap churn.
// Inhibits dataless cloud materialization prior to scan and never follows symlinks.
#[uniffi::export]
pub fn scan_directory(path: String, _max_depth: u32) -> Result<Vec<DiskItemSummary>, TTZipError> {
    scan_directory_with_error_count(path, _max_depth).map(|(items, _)| items)
}

#[cfg(target_os = "macos")]
fn scan_directory_darwin(path: &str, max_depth: u32, io_error_count: &mut u64) -> Result<Vec<DiskItemSummary>, TTZipError> {
    let c_path = std::ffi::CString::new(path.as_bytes())
        .map_err(|e| TTZipError::InvalidArgument { message: e.to_string() })?;

    let root_fd = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if root_fd < 0 {
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        return match errno {
            libc::ENOENT => Err(TTZipError::FileNotFound { path: path.to_string() }),
            libc::EACCES | libc::EPERM => Err(TTZipError::IoError { message: format!("Permission denied: {path}") }),
            libc::ENOTDIR | libc::ELOOP => Err(TTZipError::InvalidArgument { message: format!("Not a directory or symlink loop: {path}") }),
            _ => Err(TTZipError::IoError { message: format!("Failed to open directory {path}: errno {errno}") }),
        };
    }

    let root_guard = DirFdGuard::new(root_fd);
    let mut items = Vec::new();
    let effective_max_depth = if max_depth == 0 { 1 } else { max_depth };
    scan_dir_darwin_fd(root_guard.raw(), path, 1, effective_max_depth, &mut items, io_error_count)?;

    items.sort_by(|a, b| match (a.is_directory, b.is_directory) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => crate::fs::natural_sort::natural_cmp(&a.name, &b.name),
    });
    Ok(items)
}

#[cfg(target_os = "macos")]
unsafe fn decode_darwin_attr_record(
    entry_ptr: *const u8,
    parent_path: &str,
    io_error_count: &mut u64,
) -> Option<DiskItemSummary> {
    let mut field_ptr = unsafe { entry_ptr.add(std::mem::size_of::<u32>()) };
    let ret_attrs = unsafe { std::ptr::read_unaligned(field_ptr as *const libc::attribute_set_t) };
    field_ptr = unsafe { field_ptr.add(std::mem::size_of::<libc::attribute_set_t>()) };

    if ret_attrs.commonattr & ATTR_CMN_ERROR != 0 {
        let error_code = unsafe { std::ptr::read_unaligned(field_ptr as *const u32) };
        if error_code != 0 {
            *io_error_count += 1;
            return None;
        }
        field_ptr = unsafe { field_ptr.add(std::mem::size_of::<u32>()) };
    }

    let mut name = String::new();
    if ret_attrs.commonattr & libc::ATTR_CMN_NAME != 0 {
        let ref_val = unsafe { std::ptr::read_unaligned(field_ptr as *const libc::attrreference_t) };
        let name_ptr = unsafe { field_ptr.offset(ref_val.attr_dataoffset as isize) };
        let c_str = unsafe { std::ffi::CStr::from_ptr(name_ptr as *const libc::c_char) };
        name = c_str.to_string_lossy().into_owned();
        field_ptr = unsafe { field_ptr.add(std::mem::size_of::<libc::attrreference_t>()) };
    }

    if name.is_empty() || name.starts_with('.') {
        return None;
    }

    let mut obj_type = 0u32;
    if ret_attrs.commonattr & libc::ATTR_CMN_OBJTYPE != 0 {
        obj_type = unsafe { std::ptr::read_unaligned(field_ptr as *const u32) };
        field_ptr = unsafe { field_ptr.add(std::mem::size_of::<u32>()) };
    }

    let mut mtime_epoch_secs = 0i64;
    if ret_attrs.commonattr & libc::ATTR_CMN_MODTIME != 0 {
        let ts = unsafe { std::ptr::read_unaligned(field_ptr as *const libc::timespec) };
        mtime_epoch_secs = ts.tv_sec as i64;
        field_ptr = unsafe { field_ptr.add(std::mem::size_of::<libc::timespec>()) };
    }

    let mut size = 0u64;
    if ret_attrs.fileattr & libc::ATTR_FILE_DATALENGTH != 0 {
        let datalen = unsafe { std::ptr::read_unaligned(field_ptr as *const libc::off_t) };
        size = datalen as u64;
    }

    const VDIR: u32 = 2;
    let is_dir = obj_type == VDIR;
    if is_dir {
        size = 0;
    }

    let item_path = if parent_path == "/" {
        format!("/{name}")
    } else {
        format!("{}/{name}", parent_path.trim_end_matches('/'))
    };

    Some(DiskItemSummary {
        path: item_path,
        name,
        is_directory: is_dir,
        size,
        mtime_epoch_secs,
    })
}

#[cfg(target_os = "macos")]
fn try_open_subdir_guard(dir_fd: libc::c_int, name: &str) -> Option<DirFdGuard> {
    let c_name = std::ffi::CString::new(name.as_bytes()).ok()?;
    let sub_fd = unsafe {
        libc::openat(
            dir_fd,
            c_name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    (sub_fd >= 0).then(|| DirFdGuard::new(sub_fd))
}

#[cfg(target_os = "macos")]
struct DiskScanBatchDrainContext<'a> {
    dir_fd: libc::c_int,
    parent_path: &'a str,
    recurse: bool,
    subdirs: &'a mut Vec<(DirFdGuard, String)>,
    out: &'a mut Vec<DiskItemSummary>,
    io_error_count: &'a mut u64,
}

#[cfg(target_os = "macos")]
unsafe fn drain_darwin_attr_batch(
    buf_ptr: *const u8,
    buf_size: usize,
    retcount: libc::c_int,
    ctx: &mut DiskScanBatchDrainContext<'_>,
) {
    let mut entry_ptr = buf_ptr;
    let end_ptr = unsafe { entry_ptr.add(buf_size) };

    for _ in 0..retcount {
        if entry_ptr >= end_ptr {
            break;
        }
        let entry_len = unsafe { std::ptr::read_unaligned(entry_ptr as *const u32) };
        if entry_len == 0 {
            break;
        }
        let next_entry_ptr = unsafe { entry_ptr.add(entry_len as usize) };
        if next_entry_ptr > end_ptr {
            break;
        }

        if let Some(item) =
            unsafe { decode_darwin_attr_record(entry_ptr, ctx.parent_path, ctx.io_error_count) }
        {
            if item.is_directory && ctx.recurse {
                if let Some(guard) = try_open_subdir_guard(ctx.dir_fd, &item.name) {
                    ctx.subdirs.push((guard, item.path.clone()));
                } else {
                    *ctx.io_error_count += 1;
                }
            }
            ctx.out.push(item);
        }

        entry_ptr = next_entry_ptr;
    }
}

#[cfg(target_os = "macos")]
fn scan_dir_darwin_fd(
    dir_fd: libc::c_int,
    parent_path: &str,
    current_depth: u32,
    max_depth: u32,
    out: &mut Vec<DiskItemSummary>,
    io_error_count: &mut u64,
) -> Result<(), TTZipError> {
    let mut subdirs: Vec<(DirFdGuard, String)> = Vec::new();
    let recurse = current_depth < max_depth;

    SCAN_BUFFER.with(|buf_cell| -> Result<(), TTZipError> {
        let mut buf_ref = buf_cell.borrow_mut();
        let buf_ptr = buf_ref.0.as_mut_ptr();
        let buf_size = SCAN_BUFFER_SIZE;
        let mut drain_ctx = DiskScanBatchDrainContext {
            dir_fd,
            parent_path,
            recurse,
            subdirs: &mut subdirs,
            out,
            io_error_count,
        };

        loop {
            let mut attr_list: libc::attrlist = unsafe { std::mem::zeroed() };
            attr_list.bitmapcount = libc::ATTR_BIT_MAP_COUNT;
            attr_list.commonattr = libc::ATTR_CMN_RETURNED_ATTRS
                | libc::ATTR_CMN_NAME
                | libc::ATTR_CMN_OBJTYPE
                | libc::ATTR_CMN_MODTIME
                | ATTR_CMN_ERROR;
            attr_list.fileattr = libc::ATTR_FILE_DATALENGTH;

            let retcount = unsafe {
                libc::getattrlistbulk(
                    dir_fd,
                    &mut attr_list as *mut libc::attrlist as *mut libc::c_void,
                    buf_ptr as *mut libc::c_void,
                    buf_size,
                    0,
                )
            };

            if retcount < 0 {
                let err = std::io::Error::last_os_error();
                return Err(TTZipError::IoError {
                    message: format!("getattrlistbulk failed: {err}"),
                });
            }
            if retcount == 0 {
                break;
            }

            unsafe {
                drain_darwin_attr_batch(buf_ptr as *const u8, buf_size, retcount, &mut drain_ctx);
            }
        }
        Ok(())
    })?;

    for (sub_guard, sub_path) in subdirs {
        if scan_dir_darwin_fd(sub_guard.raw(), &sub_path, current_depth + 1, max_depth, out, io_error_count).is_err() {
            *io_error_count += 1;
        }
    }

    Ok(())
}

fn scan_directory_fallback(
    path: &str,
    max_depth: u32,
    io_error_count: &mut u64,
) -> Result<Vec<DiskItemSummary>, TTZipError> {
    let root = Path::new(path);
    if !root.exists() {
        return Err(TTZipError::FileNotFound { path: path.to_string() });
    }

    let mut items = Vec::new();
    let effective_max_depth = if max_depth == 0 { 1 } else { max_depth };
    scan_dir_fallback_recursive(root, 1, effective_max_depth, &mut items, io_error_count)?;

    items.sort_by(|a, b| match (a.is_directory, b.is_directory) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => crate::fs::natural_sort::natural_cmp(&a.name, &b.name),
    });

    Ok(items)
}

fn scan_dir_fallback_recursive(
    dir: &Path,
    current_depth: u32,
    max_depth: u32,
    items: &mut Vec<DiskItemSummary>,
    io_error_count: &mut u64,
) -> Result<(), TTZipError> {
    let entries = std::fs::read_dir(dir).map_err(|e| TTZipError::IoError { message: e.to_string() })?;

    for entry_res in entries {
        let Ok(entry) = entry_res else {
            *io_error_count += 1;
            continue;
        };
        let p = entry.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        if name.starts_with('.') {
            continue;
        }

        let symlink_meta = p.symlink_metadata().ok();
        let is_symlink = symlink_meta.as_ref().map(|m| m.file_type().is_symlink()).unwrap_or(false);
        let is_dir = !is_symlink && p.is_dir();
        let size = if is_dir || is_symlink { 0 } else { symlink_meta.as_ref().map(|m| m.len()).unwrap_or(0) };
        let mtime = symlink_meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        items.push(DiskItemSummary {
            path: p.to_string_lossy().to_string(),
            name,
            is_directory: is_dir,
            size,
            mtime_epoch_secs: mtime,
        });

        if is_dir
            && current_depth < max_depth
            && scan_dir_fallback_recursive(&p, current_depth + 1, max_depth, items, io_error_count)
                .is_err()
        {
            *io_error_count += 1;
        }
    }

    Ok(())
}

/// Natural string comparator exposed via UniFFI.
#[uniffi::export]
pub fn natural_compare(a: String, b: String) -> i32 {
    match crate::fs::natural_sort::natural_cmp(&a, &b) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

/// Natural sorts a list of paths exposed via UniFFI.
#[uniffi::export]
pub fn natural_sort_paths(mut items: Vec<String>) -> Vec<String> {
    crate::fs::natural_sort::natural_sort(&mut items);
    items
}

/// Normalizes and resolves a raw user input path into a canonical POSIX path.
#[uniffi::export]
pub fn sanitize_posix_path(raw_input: String, base_directory: Option<String>) -> String {
    let trimmed = strip_wrapping_quotes(&raw_input);
    if trimmed.is_empty() {
        return String::new();
    }

    let unescaped = unescape_and_expand(trimmed);
    let resolved = if unescaped.starts_with('/') {
        unescaped
    } else {
        let base = base_directory.unwrap_or_else(get_default_home);
        format!("{}/{}", base.trim_end_matches('/'), unescaped)
    };

    let std = normalize_posix_lexical(&resolved);
    if std.is_empty() { "/".to_string() } else { std }
}

/// Extracts the parent directory to query and the trailing prefix for real-time autocompletion.
#[uniffi::export]
pub fn extract_parent_and_prefix(
    raw_input: String,
    base_directory: Option<String>,
) -> UniFFIParentAndPrefix {
    let trimmed = strip_wrapping_quotes(&raw_input);
    if trimmed.is_empty() {
        let base = base_directory.unwrap_or_else(get_default_home);
        let std_base = normalize_posix_lexical(&base);
        return UniFFIParentAndPrefix {
            parent_directory: if std_base.is_empty() { "/".to_string() } else { std_base },
            prefix: String::new(),
        };
    }

    let raw_unescaped = unescape_url_and_shell(trimmed);
    let home = get_default_home();
    if raw_unescaped == "~" || raw_unescaped == "~/" {
        return UniFFIParentAndPrefix {
            parent_directory: home,
            prefix: String::new(),
        };
    }

    let unescaped = expand_tilde(&raw_unescaped);
    if unescaped == home || unescaped == format!("{}/", home) {
        return UniFFIParentAndPrefix {
            parent_directory: home,
            prefix: String::new(),
        };
    }
    let full_path = if unescaped.starts_with('/') {
        unescaped.clone()
    } else {
        let base = base_directory.unwrap_or_else(get_default_home);
        format!("{}/{}", base.trim_end_matches('/'), unescaped)
    };

    let (parent, prefix) = if unescaped.ends_with('/') {
        (normalize_posix_lexical(&full_path), String::new())
    } else if unescaped == "." || unescaped.ends_with("/.") {
        (normalize_posix_lexical(&posix_parent(&full_path)), ".".to_string())
    } else {
        let (p, pre) = posix_parent_and_last(&full_path);
        (normalize_posix_lexical(&p), pre)
    };

    UniFFIParentAndPrefix {
        parent_directory: if parent.is_empty() { "/".to_string() } else { parent },
        prefix,
    }
}

/// Fast path autocompletion query based on directory scanning and prefix matching.
#[uniffi::export]
pub fn autocomplete_disk_path(
    raw_input: String,
    base_directory: String,
    max_results: u32,
) -> Vec<PathSuggestionItem> {
    let input = raw_input.trim();
    if input.is_empty() { return Vec::new(); }

    let parsed = extract_parent_and_prefix(raw_input, Some(base_directory));
    let search_dir = Path::new(&parsed.parent_directory);
    let prefix = parsed.prefix;
    if !search_dir.exists() || !search_dir.is_dir() { return Vec::new(); }

    let archive_exts = [
        "zip", "7z", "tar", "gz", "bz2", "xz", "zst", "rar", "dmg", "iso", "wim", "aar", "cbr", "cab", "lz4", "br"
    ];

    let mut matches = Vec::new();
    if let Ok(entries) = std::fs::read_dir(search_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !prefix.starts_with('.') && name.starts_with('.') { continue; }
            if prefix.is_empty() || name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                let p = entry.path();
                let is_dir = p.is_dir();
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                matches.push(PathSuggestionItem {
                    full_path: p.to_string_lossy().to_string(),
                    display_name: name,
                    is_directory: is_dir,
                    is_archive: archive_exts.contains(&ext.as_str()),
                });
                if matches.len() >= (max_results as usize) * 3 { break; }
            }
        }
    }

    matches.sort_by(|a, b| match (a.is_directory, b.is_directory) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => crate::fs::natural_sort::natural_cmp(&a.display_name, &b.display_name),
    });
    matches.truncate(max_results as usize);
    matches
}

// MARK: - Internal Helper Functions

fn strip_wrapping_quotes(s: &str) -> &str {
    let t = s.trim();
    if (t.starts_with('"') && t.ends_with('"') && t.len() >= 2)
        || (t.starts_with('\'') && t.ends_with('\'') && t.len() >= 2)
    {
        t[1..t.len() - 1].trim()
    } else {
        t
    }
}

fn unescape_url_and_shell(input: &str) -> String {
    let url_stripped = if let Some(s) = input.strip_prefix("file://localhost") {
        percent_decode(s)
    } else if let Some(s) = input.strip_prefix("file://") {
        percent_decode(s)
    } else {
        input.to_string()
    };
    unescape_shell_backslashes(&url_stripped)
}

fn unescape_and_expand(input: &str) -> String {
    expand_tilde(&unescape_url_and_shell(input))
}

fn percent_decode(input: &str) -> String {
    let mut bytes = Vec::with_capacity(input.len());
    let mut chars = input.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            if let (Some(h1), Some(h2)) = (chars.next(), chars.next()) {
                if let (Some(v1), Some(v2)) = (hex_val(h1), hex_val(h2)) {
                    bytes.push((v1 << 4) | v2);
                    continue;
                }
                bytes.extend([b'%', h1, h2]);
            } else {
                bytes.push(b'%');
            }
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8(bytes).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

#[inline]
fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn unescape_shell_backslashes(path: &str) -> String {
    if !path.contains('\\') { return path.to_string(); }
    let mut res = String::with_capacity(path.len());
    let mut esc = false;
    for c in path.chars() {
        if esc { res.push(c); esc = false; }
        else if c == '\\' { esc = true; }
        else { res.push(c); }
    }
    if esc { res.push('\\'); }
    res
}

fn get_default_home() -> String {
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() && home != "/" && home != "/Users" && home != "/home" {
            return home;
        }
    }
    #[cfg(unix)]
    unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if !pw.is_null() && !(*pw).pw_dir.is_null() {
            if let Ok(s) = std::ffi::CStr::from_ptr((*pw).pw_dir).to_str() {
                if !s.is_empty() {
                    return s.to_string();
                }
            }
        }
    }
    std::env::var("HOME").unwrap_or_else(|_| {
        if cfg!(target_os = "macos") { "/Users".into() } else { "/home".into() }
    })
}

fn expand_tilde(path: &str) -> String {
    if path == "~" {
        get_default_home()
    } else if let Some(s) = path.strip_prefix("~/") {
        format!("{}/{}", get_default_home().trim_end_matches('/'), s)
    } else if let Some(s) = path.strip_prefix('~') {
        let (user, rest) = match s.find('/') {
            Some(i) => (&s[..i], &s[i..]),
            None => (s, ""),
        };
        let pfx = if cfg!(target_os = "macos") { "/Users" } else { "/home" };
        format!("{}/{}{}", pfx, user, rest)
    } else {
        path.to_string()
    }
}

fn normalize_posix_lexical(path: &str) -> String {
    if path.is_empty() { return String::new(); }
    let is_abs = path.starts_with('/');
    let mut comps: Vec<&str> = Vec::new();
    for comp in path.split('/') {
        match comp {
            "" | "." => continue,
            ".." => {
                if let Some(last) = comps.last() {
                    if *last != ".." { comps.pop(); } else if !is_abs { comps.push(".."); }
                } else if !is_abs { comps.push(".."); }
            }
            other => comps.push(other),
        }
    }
    if is_abs {
        if comps.is_empty() { "/".into() } else { format!("/{}", comps.join("/")) }
    } else if comps.is_empty() { ".".into() } else { comps.join("/") }
}

fn posix_parent_and_last(path: &str) -> (String, String) {
    let t = path.trim_end_matches('/');
    if let Some(idx) = t.rfind('/') {
        let parent = if idx == 0 { "/".into() } else { t[..idx].into() };
        (parent, t[idx + 1..].into())
    } else {
        (".".into(), t.into())
    }
}

fn posix_parent(path: &str) -> String {
    posix_parent_and_last(path).0
}

#[cfg(test)]
#[path = "disk_scanner_tests.rs"]
mod tests;
