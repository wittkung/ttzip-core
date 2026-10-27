// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! macOS native system-level defensive invariants and resource optimization layer.
//!
//! Provides zero-prompt Full Disk Access (FDA) probing, VFS dataless cloud materialization
//! inhibition, and Darwin malloc zone dirty cache purging.

#[cfg(target_os = "macos")]
extern "C" {
    fn setiopolicy_np(iotype: libc::c_int, scope: libc::c_int, policy: libc::c_int) -> libc::c_int;
    fn malloc_zone_pressure_relief(zone: *mut libc::c_void, goal: libc::size_t) -> libc::size_t;
}

/// Silently probes whether the process has macOS Full Disk Access (FDA) without triggering a privacy prompt.
///
/// On macOS, attempts to open `/Library/Application Support/com.apple.TCC/TCC.db` in read-only mode.
/// Access to this system database is guarded by Full Disk Access / System Integrity Protection.
/// If access is permitted, returns `true`; if `EPERM`, `EACCES`, or any error occurs, returns `false`
/// without causing macOS to display a user consent dialog.
///
/// On non-macOS platforms, returns `false`.
#[uniffi::export]
pub fn probe_full_disk_access_silent() -> bool {
    #[cfg(target_os = "macos")]
    {
        let tcc_path = b"/Library/Application Support/com.apple.TCC/TCC.db\0";
        let fd = unsafe {
            libc::open(
                tcc_path.as_ptr() as *const libc::c_char,
                libc::O_RDONLY | libc::O_CLOEXEC,
            )
        };
        if fd >= 0 {
            unsafe {
                libc::close(fd);
            }
            true
        } else {
            false
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Applies VFS I/O policy to inhibit background automatic download of dataless (cloud/iCloud) files.
///
/// On macOS, invokes the Darwin kernel API `setiopolicy_np(3, 0, 1)`:
/// - `IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES = 3`
/// - `IOPOL_SCOPE_PROCESS = 0`
/// - `IOPOL_MATERIALIZE_DATALESS_FILES_OFF = 1`
///
/// On non-macOS platforms, this function is a safe no-op.
#[uniffi::export]
pub fn apply_vfs_dataless_materialization_policy() {
    #[cfg(target_os = "macos")]
    {
        const IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES: libc::c_int = 3;
        const IOPOL_SCOPE_PROCESS: libc::c_int = 0;
        const IOPOL_MATERIALIZE_DATALESS_FILES_OFF: libc::c_int = 1;

        unsafe {
            let _ = setiopolicy_np(
                IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES,
                IOPOL_SCOPE_PROCESS,
                IOPOL_MATERIALIZE_DATALESS_FILES_OFF,
            );
        }
    }
}

/// Requests macOS malloc zones to release unused, dirty, or cached heap pages back to the kernel.
///
/// On macOS, invokes `malloc_zone_pressure_relief(NULL, 0)`, which signals all malloc zones
/// to trim cached pages and reduce resident memory footprint.
///
/// On non-macOS platforms, this function is a safe no-op.
#[uniffi::export]
pub fn relieve_malloc_pressure() {
    #[cfg(target_os = "macos")]
    {
        unsafe {
            let _ = malloc_zone_pressure_relief(std::ptr::null_mut(), 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe_full_disk_access_silent_does_not_panic() {
        let _has_fda = probe_full_disk_access_silent();
    }

    #[test]
    fn test_apply_vfs_dataless_materialization_policy_safe() {
        apply_vfs_dataless_materialization_policy();
    }

    #[test]
    fn test_relieve_malloc_pressure_safe() {
        relieve_malloc_pressure();
    }
}
