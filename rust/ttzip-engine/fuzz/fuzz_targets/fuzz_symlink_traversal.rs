// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Task T046: Coverage-Guided Fuzz Target for Symlink Traversal & Path Sanitization (`fuzz_symlink_traversal`).
//!
//! Thoroughly validates archive extraction path sanitization against sophisticated adversarial exploits:
//! - Parent directory traversal sequences (`../`, `..\\`, mixed `/../\\../`).
//! - Absolute POSIX roots, Windows drive letters (`C:`, `D:\`), UNC namespaces, Win32 device namespaces (`\\?\`, `\\.\`).
//! - Symlink redirection loops, self-referential cycles, and deep symlink chains exceeding depth boundaries.
//! - Null byte injection attacks (`\0`).
//! - Unicode and UTF-8 normalization exploits (fullwidth slashes `／`, dots `‥`, `．`, percent-encoded `%2e%2e`).
//! - Asserts all resolved extraction targets are strictly enclosed within the destination sandbox directory; 0 path escapes.

use std::collections::{HashMap, HashSet};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use ttzip_engine::fs::safe_extract::sanitize_and_validate_path;
use ttzip_engine::security::{
    enclosed_name, simplified_components, validate_symlink_target, PathTraversalProtectionGuard,
    SandboxEscapingGuard,
};
use ttzip_engine::types::TTZipStatus;

/// Simulated destination jail root directory for sandboxed path testing.
const SANDBOX_JAIL_ROOT: &str = "/var/run/ttzip_sandbox_jail";

/// Maximum allowable symlink evaluation depth.
const MAX_SYMLINK_RECURSION_DEPTH: usize = 8;

/// Error conditions produced during virtual symlink resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraversalFuzzError {
    /// Symlink redirection loop or cycle detected.
    SymlinkLoopDetected { path: String, chain: Vec<String> },
    /// Symlink resolution exceeded maximum evaluation depth.
    DepthLimitExceeded { path: String, depth: usize },
    /// Symlink target escapes destination sandbox boundary.
    SandboxEscape { path: String, target: String },
    /// Null byte injection detected in path segment.
    NullByteDetected(String),
    /// Invalid or malformed path.
    InvalidPath(String),
}

/// In-memory virtual VFS symlink resolver testing graph cycle interception.
pub struct VirtualSymlinkJail {
    symlink_table: HashMap<String, String>,
    jail_root: PathBuf,
    guard: PathTraversalProtectionGuard,
    escaping_guard: SandboxEscapingGuard,
}

impl Default for VirtualSymlinkJail {
    fn default() -> Self {
        Self::new(SANDBOX_JAIL_ROOT)
    }
}

impl VirtualSymlinkJail {
    /// Creates a new virtual jail with specified destination root.
    #[must_use]
    pub fn new(jail_root: impl Into<PathBuf>) -> Self {
        let root = jail_root.into();
        Self {
            symlink_table: HashMap::new(),
            guard: PathTraversalProtectionGuard::strict(),
            escaping_guard: SandboxEscapingGuard::with_jail_root(&root),
            jail_root: root,
        }
    }

    /// Registers a virtual symlink entry.
    pub fn register_symlink(&mut self, link_path: &str, target_path: &str) {
        self.symlink_table
            .insert(link_path.to_string(), target_path.to_string());
    }

    /// Resolves a candidate symlink chain with cycle detection and jail containment.
    ///
    /// # Errors
    /// Returns `TraversalFuzzError` if a loop is detected, depth limit exceeded, or path escapes jail.
    pub fn resolve_symlink(
        &self,
        start_link: &str,
        depth_limit: usize,
    ) -> Result<PathBuf, TraversalFuzzError> {
        if start_link.contains('\0') {
            return Err(TraversalFuzzError::NullByteDetected(start_link.to_string()));
        }

        let mut current = start_link.to_string();
        let mut visited = HashSet::new();
        let mut chain = Vec::new();
        let mut steps = 0;

        loop {
            if steps >= depth_limit {
                return Err(TraversalFuzzError::DepthLimitExceeded {
                    path: current,
                    depth: steps,
                });
            }

            if !visited.insert(current.clone()) {
                chain.push(current.clone());
                return Err(TraversalFuzzError::SymlinkLoopDetected {
                    path: current,
                    chain,
                });
            }

            chain.push(current.clone());
            steps += 1;

            if let Some(target) = self.symlink_table.get(&current) {
                // Validate target against production symlink defense
                let validation = validate_symlink_target(&self.jail_root, target, depth_limit);
                match validation {
                    Ok(resolved_target) => {
                        // Assert that resolved target is inside jail root
                        if !resolved_target.starts_with(&self.jail_root) {
                            return Err(TraversalFuzzError::SandboxEscape {
                                path: current,
                                target: target.clone(),
                            });
                        }
                        current = target.clone();
                    }
                    Err(TTZipStatus::ErrSecurityViolation) => {
                        return Err(TraversalFuzzError::SandboxEscape {
                            path: current,
                            target: target.clone(),
                        });
                    }
                    Err(other) => {
                        return Err(TraversalFuzzError::InvalidPath(format!("{:?}", other)));
                    }
                }
            } else {
                // Terminal target reached - sanitize using production PathTraversalProtectionGuard
                let sanitized = self
                    .guard
                    .sanitize_path(&current)
                    .map_err(|e| TraversalFuzzError::InvalidPath(format!("{:?}", e)))?;

                let full_path = self.jail_root.join(sanitized);
                let validated = self
                    .escaping_guard
                    .validate_path(&full_path)
                    .map_err(|e| TraversalFuzzError::SandboxEscape {
                        path: current.clone(),
                        target: format!("{:?}", e),
                    })?;

                return Ok(validated);
            }
        }
    }
}

/// Tests arbitrary path strings across all production sanitization engines.
fn evaluate_path_sanitization_invariants(dest_dir: &Path, raw_path: &str) {
    // 1. Production `sanitize_and_validate_path`
    if let Ok(resolved) = sanitize_and_validate_path(dest_dir, raw_path) {
        assert!(
            resolved.starts_with(dest_dir),
            "INVARIANT ESCAPE: sanitize_and_validate_path produced {:?} escaping {:?}",
            resolved,
            dest_dir
        );
        let has_parent_dir = resolved
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir));
        assert!(
            !has_parent_dir,
            "INVARIANT ESCAPE: resolved path contains ParentDir ('..'): {:?}",
            resolved
        );
        let path_str = resolved.to_string_lossy();
        assert!(
            !path_str.contains('\0'),
            "INVARIANT ESCAPE: resolved path contains null-byte: {:?}",
            resolved
        );
    }

    // 2. Production `enclosed_name`
    if let Some(enclosed) = enclosed_name(raw_path) {
        let full = dest_dir.join(&enclosed);
        assert!(
            full.starts_with(dest_dir),
            "INVARIANT ESCAPE: enclosed_name produced {:?} escaping {:?}",
            full,
            dest_dir
        );
        assert!(
            !enclosed.components().any(|c| matches!(c, std::path::Component::ParentDir)),
            "enclosed_name contains ParentDir ('..'): {:?}",
            enclosed
        );
        let s = enclosed.to_string_lossy();
        assert!(!s.contains('\0'), "enclosed_name contains null byte: {:?}", s);
    }

    // 3. Production `simplified_components`
    if let Some(simplified) = simplified_components(raw_path) {
        let full = dest_dir.join(&simplified);
        assert!(
            full.starts_with(dest_dir),
            "INVARIANT ESCAPE: simplified_components produced {:?} escaping {:?}",
            full,
            dest_dir
        );
        assert!(
            !simplified.components().any(|c| matches!(c, std::path::Component::ParentDir)),
            "simplified_components contains ParentDir ('..'): {:?}",
            simplified
        );
        let s = simplified.to_string_lossy();
        assert!(!s.contains('\0'), "simplified_components contains null: {:?}", s);
    }

    // 4. Production `PathTraversalProtectionGuard`
    let guard = PathTraversalProtectionGuard::strict();
    if let Ok(sanitized) = guard.sanitize_path(raw_path) {
        let full = dest_dir.join(&sanitized);
        assert!(
            full.starts_with(dest_dir),
            "INVARIANT ESCAPE: PathTraversalProtectionGuard produced {:?} escaping {:?}",
            full,
            dest_dir
        );
        assert!(
            !Path::new(&sanitized).components().any(|c| matches!(c, std::path::Component::ParentDir)),
            "PathTraversalProtectionGuard contains ParentDir: {:?}",
            sanitized
        );
        assert!(!sanitized.contains('\0'));
    }

    // 5. Production `validate_symlink_target`
    if let Ok(resolved) = validate_symlink_target(dest_dir, raw_path, MAX_SYMLINK_RECURSION_DEPTH) {
        assert!(
            resolved.starts_with(dest_dir),
            "INVARIANT ESCAPE: validate_symlink_target produced {:?} escaping {:?}",
            resolved,
            dest_dir
        );
    }
}

/// Synthesizes structured adversarial paths and symlink graphs from fuzz input bytes.
pub fn fuzz_target_symlink_traversal(data: &[u8]) {
    let dest_dir = Path::new(SANDBOX_JAIL_ROOT);

    let result = catch_unwind(AssertUnwindSafe(|| {
        if data.is_empty() {
            return;
        }

        // 1. Direct raw string slice test
        if let Ok(raw_str) = std::str::from_utf8(data) {
            evaluate_path_sanitization_invariants(dest_dir, raw_str);
        } else {
            // Lossy UTF-8 representation test
            let lossy_str = String::from_utf8_lossy(data);
            evaluate_path_sanitization_invariants(dest_dir, &lossy_str);
        }

        // 2. Synthesized path permutations (joining segments from tokens)
        let tokens: &[&str] = &[
            "..", "../", "..\\", "/", "\\", "C:", "D:\\", "\\\\server\\share",
            "\\\\?\\C:\\", "\\\\.\\PhysicalDrive0", "\0", "foo\0bar", "secret", "passwd",
            "／", "＼", "‥", "．", "%2e%2e", "%2f", "%5c", "%00", "a", "b", "c",
        ];

        let mut path_builder = String::new();
        for &byte in data.iter().take(16) {
            let idx = (byte as usize) % tokens.len();
            path_builder.push_str(tokens[idx]);
            if (byte & 1) == 0 {
                path_builder.push('/');
            } else if (byte & 2) == 0 {
                path_builder.push('\\');
            }
        }
        evaluate_path_sanitization_invariants(dest_dir, &path_builder);

        // 3. Virtual symlink graph cycle and depth fuzzing
        if data.len() >= 4 {
            let mut jail = VirtualSymlinkJail::new(dest_dir);

            // Construct up to 4 virtual symlinks from data chunks
            let chunks = data.chunks_exact(4);
            let mut link_names = Vec::new();

            for (i, chunk) in chunks.enumerate().take(6) {
                let from = format!("link_{}", i);
                let target_mode = chunk[0] % 5;
                let target = match target_mode {
                    0 => format!("link_{}", (chunk[1] as usize) % 6), // Reference another link (potential cycle)
                    1 => format!("../outside_{}.txt", chunk[2]),       // Escaping relative target
                    2 => format!("/etc/passwd_{}", chunk[3]),          // Absolute path target
                    3 => format!("valid_sub/target_{}.bin", chunk[2]), // Enclosed legitimate target
                    _ => format!("nested/link_{}", (chunk[1] as usize) % 6),
                };
                jail.register_symlink(&from, &target);
                link_names.push(from);
            }

            // Attempt resolution on all registered links
            for link in link_names {
                let res = jail.resolve_symlink(&link, MAX_SYMLINK_RECURSION_DEPTH);
                if let Ok(target) = res {
                    assert!(
                        target.starts_with(dest_dir),
                        "INVARIANT ESCAPE: Symlink jail produced target {:?} outside {:?}",
                        target,
                        dest_dir
                    );
                }
            }
        }
    }));

    if let Err(panic_payload) = result {
        panic!(
            "CRITICAL SECURITY FAILURE: Panic caught during symlink traversal fuzzing: {:?}",
            panic_payload
        );
    }
}

fn main() {
    let static_malicious_corpus = [
        // Classic ZipSlip
        "../../../../../../etc/passwd",
        r"..\..\..\..\..\..\Windows\System32\cmd.exe",
        "a/b/c/../../../../../../etc/shadow",
        "sub/dir/../../../secret.key",
        r"sub\dir\..\..\..\secret.key",
        // Absolute POSIX
        "/etc/passwd",
        "///root/secret",
        "/private/var/root/.ssh/id_rsa",
        // Absolute Windows & UNC
        r"C:\Windows\System32\cmd.exe",
        "C:/autoexec.bat",
        r"\\127.0.0.1\c$\payload.exe",
        r"\\?\C:\tools\malware.exe",
        r"\\.\PhysicalDrive0",
        // Null byte injection
        "normal.txt\0/etc/passwd",
        "\0../../escape",
        "doc.pdf\0.exe",
        // Unicode & UTF-8 normalization exploits
        "foo／bar／..／etc／passwd",
        "‥/‥/outside",
        "．．/．．/secret",
        "%2e%2e/%2e%2e/escape",
        // Chained traversals
        "....//....//etc/passwd",
        r"..\/..\/secret",
        "file:///etc/passwd",
    ];

    for &payload in &static_malicious_corpus {
        fuzz_target_symlink_traversal(payload.as_bytes());
    }

    // Direct symlink cycle verification
    let mut jail = VirtualSymlinkJail::new(SANDBOX_JAIL_ROOT);
    jail.register_symlink("loop_a", "loop_b");
    jail.register_symlink("loop_b", "loop_a");
    let loop_res = jail.resolve_symlink("loop_a", MAX_SYMLINK_RECURSION_DEPTH);
    assert!(
        matches!(loop_res, Err(TraversalFuzzError::SymlinkLoopDetected { .. })),
        "Expected symlink loop interception, got: {:?}",
        loop_res
    );

    // Direct self-loop verification
    jail.register_symlink("self_loop", "self_loop");
    let self_res = jail.resolve_symlink("self_loop", MAX_SYMLINK_RECURSION_DEPTH);
    assert!(
        matches!(self_res, Err(TraversalFuzzError::SymlinkLoopDetected { .. })),
        "Expected self-loop interception, got: {:?}",
        self_res
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symlink_cycle_interception() {
        let mut jail = VirtualSymlinkJail::new(SANDBOX_JAIL_ROOT);
        jail.register_symlink("node1", "node2");
        jail.register_symlink("node2", "node3");
        jail.register_symlink("node3", "node1");

        let res = jail.resolve_symlink("node1", 8);
        assert!(matches!(res, Err(TraversalFuzzError::SymlinkLoopDetected { .. })));
    }

    #[test]
    fn test_symlink_depth_limit() {
        let mut jail = VirtualSymlinkJail::new(SANDBOX_JAIL_ROOT);
        for i in 0..15 {
            jail.register_symlink(&format!("chain_{}", i), &format!("chain_{}", i + 1));
        }

        let res = jail.resolve_symlink("chain_0", 8);
        assert!(matches!(res, Err(TraversalFuzzError::DepthLimitExceeded { .. })));
    }

    #[test]
    fn test_symlink_sandbox_escape_rejected() {
        let mut jail = VirtualSymlinkJail::new(SANDBOX_JAIL_ROOT);
        jail.register_symlink("escape_link", "../../etc/shadow");

        let res = jail.resolve_symlink("escape_link", 8);
        assert!(matches!(res, Err(TraversalFuzzError::SandboxEscape { .. })));
    }
}
