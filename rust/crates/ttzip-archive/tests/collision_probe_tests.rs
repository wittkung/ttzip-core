// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use std::fs;
use std::path::Path;
use std::process::Command;
use ttzip_archive::fs::collision::fs_ops::{LibcFsOps, FsOps};
use ttzip_archive::fs::collision::probe::{probe_destination, ProbeResult};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::sync::Mutex;

static SERIAL_TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_probe_default_apfs() {
    let _lock = SERIAL_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let fs_ops = LibcFsOps;
    let c_path = CString::new(tmp.path().as_os_str().as_bytes()).unwrap();
    
    // SAFETY: path is null-terminated, flags are valid
    let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) };
    assert!(fd >= 0, "failed to open dir");
    
    // Check APFS capabilities on /tmp
    let caps = fs_ops.probe_vol_caps(tmp.path()).unwrap();
    assert!(caps.rename_swap, "APFS on /tmp must support rename_swap");
    assert!(caps.rename_excl, "APFS on /tmp must support rename_excl");

    unsafe { libc::close(fd) };
}

#[test]
fn test_probe_case_sensitive_apfs() {
    let _lock = SERIAL_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let img_path = tmp.path().join("cs_test.sparseimage");
    
    let output = Command::new("hdiutil")
        .args(["create", "-fs", "Case-sensitive APFS", "-size", "10m", "-type", "SPARSE", img_path.to_str().unwrap()])
        .output();
        
    if output.is_err() || !output.as_ref().unwrap().status.success() {
        println!("Skipping case-sensitive APFS test: hdiutil failed or unavailable");
        return;
    }
    
    let attach = Command::new("hdiutil")
        .args(["attach", img_path.to_str().unwrap()])
        .output().unwrap();
        
    let stdout = String::from_utf8_lossy(&attach.stdout);
    let mut mount_point = None;
    for line in stdout.lines() {
        if line.contains("/Volumes/") {
            mount_point = Some(line.split("/Volumes/").nth(1).unwrap().trim().to_string());
        }
    }
    
    if let Some(mp) = mount_point {
        let vol_path = format!("/Volumes/{}", mp);
        
        struct DetachGuard(String);
        impl Drop for DetachGuard {
            fn drop(&mut self) {
                let _ = Command::new("hdiutil").args(["detach", &self.0, "-force"]).output();
            }
        }
        let _guard = DetachGuard(vol_path.clone());
        
        let path = Path::new(&vol_path).join("CaseTest.txt");
        fs::write(&path, "case_data").unwrap();
        
        
        let c_vol = CString::new(vol_path.as_bytes()).unwrap();
        
        // SAFETY: path is null-terminated, flags are valid
        let dirfd = unsafe { libc::open(c_vol.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) };
        assert!(dirfd >= 0);
        
        let fs_ops = LibcFsOps;
        let res_exact = probe_destination(&fs_ops, dirfd, Path::new("CaseTest.txt")).unwrap();
        match res_exact {
            ProbeResult::Item(_meta, name) => {
                assert_eq!(name, "CaseTest.txt");
            }
            _ => panic!("Expected Item for exact case match"),
        }
        
        let res_lowercase = probe_destination(&fs_ops, dirfd, Path::new("casetest.txt")).unwrap();
        assert!(matches!(res_lowercase, ProbeResult::Absent), "Lowercase path must be Absent on case-sensitive APFS");
        
        // SAFETY: closing a valid opened file descriptor
        unsafe { libc::close(dirfd) };
    }
}

#[test]
fn test_probe_exfat_capabilities() {
    let _lock = SERIAL_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let img_path = tmp.path().join("exfat_test.sparseimage");
    
    let output = std::process::Command::new("hdiutil")
        .args(["create", "-fs", "exFAT", "-size", "10m", "-type", "SPARSE", img_path.to_str().unwrap()])
        .output();
        
    if output.is_err() || !output.as_ref().unwrap().status.success() {
        println!("Skipping exFAT test: hdiutil failed");
        return;
    }
    
    let attach = std::process::Command::new("hdiutil")
        .args(["attach", img_path.to_str().unwrap()])
        .output().unwrap();
        
    let stdout = String::from_utf8_lossy(&attach.stdout);
    let mut mount_point = None;
    for line in stdout.lines() {
        if line.contains("/Volumes/") {
            mount_point = Some(line.split("/Volumes/").nth(1).unwrap().trim().to_string());
        }
    }
    
    if let Some(mp) = mount_point {
        let vol_path = format!("/Volumes/{}", mp);
        
        struct DetachGuard(String);
        impl Drop for DetachGuard {
            fn drop(&mut self) {
                let _ = Command::new("hdiutil").args(["detach", &self.0, "-force"]).output();
            }
        }
        let _guard = DetachGuard(vol_path.clone());
        
        let fs_ops = ttzip_archive::fs::collision::fs_ops::LibcFsOps;
        let caps = fs_ops.probe_vol_caps(std::path::Path::new(&vol_path)).unwrap();
        assert!(!caps.rename_swap);
        assert!(!caps.rename_excl);
    }
}

mod support;
use support::collision_fixtures::CollisionFixture;
use std::os::unix::fs::PermissionsExt;
use ttzip_archive::fs::collision::{StagingArea, sweep_stale};

#[test]
fn test_staging_area_modes() {
    let _lock = SERIAL_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let fixture = CollisionFixture::new().unwrap();
    let area = StagingArea::create(fixture.root.path()).unwrap();
    
    let meta = fs::metadata(&area.staging_dir).unwrap();
    assert_eq!(meta.permissions().mode() & 0o777, 0o700);
    
    let meta_payload = fs::metadata(area.payload_dir().0).unwrap();
    assert_eq!(meta_payload.permissions().mode() & 0o777, 0o700);
    
    let meta_displaced = fs::metadata(area.displaced_dir()).unwrap();
    assert_eq!(meta_displaced.permissions().mode() & 0o777, 0o700);
    area.commit();
}

#[test]
fn test_sweep_stale_unit() {
    let _lock = SERIAL_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let fixture = CollisionFixture::new().unwrap();
    let dest = fixture.root.path();
    
    let dead_pid = 999999;
    let dead_dir = dest.join(format!(".ttzip_staging_{}_1_123", dead_pid));
    fs::create_dir(&dead_dir).unwrap();
    
    let live_pid = std::process::id();
    let live_dir = dest.join(format!(".ttzip_staging_{}_2_456", live_pid));
    fs::create_dir(&live_dir).unwrap();
    
    let dead_file = dest.join(format!(".ttzip_staging_{}_3_789", dead_pid));
    fs::write(&dead_file, "data").unwrap();
    
    let swept = sweep_stale(dest);
    assert_eq!(swept, 1);
    
    assert!(!dead_dir.exists());
    assert!(live_dir.exists());
    assert!(dead_file.exists());
}

#[test]
fn test_staging_area_fd_leak() {
    let _lock = SERIAL_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let fixture = CollisionFixture::new().unwrap();
    fixture.assert_fd_leak(|dest| {
        let area = StagingArea::create(dest).unwrap();
        area.commit();
        let _ = sweep_stale(dest);
    });
}

#[test]
fn test_collision_fixtures_helpers() {
    let _lock = SERIAL_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let fixture = CollisionFixture::new().unwrap();
    fixture.create_file("sub/file.txt", 42).unwrap();
    assert!(fixture.root.path().join("sub/file.txt").exists());
    assert_eq!(fs::metadata(fixture.root.path().join("sub/file.txt")).unwrap().len(), 42);

    fixture.create_dir("nested/deep/dir").unwrap();
    assert!(fixture.root.path().join("nested/deep/dir").is_dir());

    fixture.create_symlink("sub/file.txt", "nested/deep/symlink.txt").unwrap();
    let symlink_path = fixture.root.path().join("nested/deep/symlink.txt");
    assert!(fs::symlink_metadata(&symlink_path).unwrap().file_type().is_symlink());
}
