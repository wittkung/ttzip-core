// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use super::*;
use std::cell::RefCell;
use std::os::fd::AsRawFd;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

struct ScopedCloseErrnoInjection;

impl ScopedCloseErrnoInjection {
    fn install(errno: i32) -> Self {
        TEST_CLOSE_INJECTION.with(|slot| *slot.borrow_mut() = Some((errno, 0)));
        Self
    }

    fn calls() -> usize {
        TEST_CLOSE_INJECTION.with(|slot| slot.borrow().map_or(0, |(_, c)| c))
    }
}

impl Drop for ScopedCloseErrnoInjection {
    fn drop(&mut self) {
        TEST_CLOSE_INJECTION.with(|slot| *slot.borrow_mut() = None);
    }
}

/// Acquires a non-blocking exclusive `flock(2)` on `target` and opens a second independent
/// observer [`File`] on the same path, bound to `target`'s kernel open file description.
fn attach_open_description_observer(path: &Path, target: &File) -> File {
    let rc = unsafe { libc::flock(target.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    assert_eq!(rc, 0, "exclusive flock on target file description must succeed");
    File::open(path).expect("open observer file description")
}

/// Returns `true` while the target's kernel open file description still holds its exclusive
/// `flock(2)`, and `false` once that open file description has been closed by the kernel.
fn is_target_description_open(observer: &File) -> bool {
    let rc = unsafe { libc::flock(observer.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc == 0 {
        let _ = unsafe { libc::flock(observer.as_raw_fd(), libc::LOCK_UN) };
        false
    } else {
        true
    }
}

#[test]
fn real_regular_file_close_and_proof_succeeds_once() {
    assert_eq!(
        CloseScopePrecondition::default(),
        CloseScopePrecondition::ConservativeUnverifiedScope
    );
    let dir = tempfile::tempdir().expect("tempdir");
    let path_a = dir.path().join("real_close_none.bin");
    let mut file_a = File::create(&path_a).expect("create file_a");
    file_a.write_all(b"ttzip-b1").expect("write file_a");
    let obs_a = attach_open_description_observer(&path_a, &file_a);
    let raw_a = file_a.as_raw_fd();
    assert!(is_target_description_open(&obs_a));

    let proof_a = finish_and_close_file_for_proof(
        file_a,
        (11, 22),
        Ok(()),
        None,
        FileFinishSyncPolicy::None,
        CloseScopePrecondition::ConservativeUnverifiedScope,
    )
    .expect("normal finish must succeed");
    assert_eq!(proof_a.closed_raw_fd, raw_a);
    assert_eq!(proof_a.confirmed_identity, (11, 22));
    assert!(!proof_a.sync_performed);
    assert_eq!(proof_a.release_status, DescriptorReleaseStatus::ConfirmedReleased);
    assert!(!is_target_description_open(&obs_a));

    let path_b = dir.path().join("real_close_sync.bin");
    let mut file_b = File::create(&path_b).expect("create file_b");
    file_b.write_all(b"ttzip-b1-sync").expect("write file_b");
    let obs_b = attach_open_description_observer(&path_b, &file_b);
    let raw_b = file_b.as_raw_fd();
    assert!(is_target_description_open(&obs_b));
    let closed_b = explicit_finish_and_close_file_production(
        file_b,
        Ok(()),
        None,
        FileFinishSyncPolicy::SyncAll,
        CloseScopePrecondition::for_current_target_unique_regular_file(),
    )
    .expect("sync_all + close must succeed");
    assert_eq!(closed_b, raw_b);
    assert!(!is_target_description_open(&obs_b));
}

#[test]
fn classify_macos_applicable_eio_is_confirmed_released_but_still_error_without_proof() {
    let eio_res: io::Result<()> = Err(io::Error::from_raw_os_error(libc::EIO));
    #[cfg(target_os = "macos")]
    let expected = DescriptorReleaseStatus::ConfirmedReleased;
    #[cfg(not(target_os = "macos"))]
    let expected = DescriptorReleaseStatus::ReleaseUnconfirmed;
    assert_eq!(
        classify_close_release_status_for_scope(
            &eio_res,
            CloseScopePrecondition::MacOsUniqueNonGuardedOrdinaryVnode,
        ),
        expected
    );

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("macos_eio.bin");
    let file = File::create(&path).expect("create");
    let obs = attach_open_description_observer(&path, &file);
    let _inj = ScopedCloseErrnoInjection::install(libc::EIO);
    let res = finish_and_close_file_for_proof(
        file,
        (1, 2),
        Ok(()),
        None,
        FileFinishSyncPolicy::None,
        CloseScopePrecondition::MacOsUniqueNonGuardedOrdinaryVnode,
    );
    assert_eq!(ScopedCloseErrnoInjection::calls(), 1);
    assert!(!is_target_description_open(&obs));
    let err = res.expect_err("close EIO must still be an error and never yield proof");
    assert_eq!(err.release_status, expected);
    assert_eq!(err.close_error.as_ref().and_then(io::Error::raw_os_error), Some(libc::EIO));
}

#[test]
fn classify_eintr_ebadf_and_unknown_errors_are_release_unconfirmed() {
    for errno in [libc::EINTR, libc::EBADF, libc::ENOSPC, libc::EACCES, 9999] {
        let res: io::Result<()> = Err(io::Error::from_raw_os_error(errno));
        for scope in [
            CloseScopePrecondition::MacOsUniqueNonGuardedOrdinaryVnode,
            CloseScopePrecondition::ConservativeUnverifiedScope,
        ] {
            assert_eq!(
                classify_close_release_status_for_scope(&res, scope),
                DescriptorReleaseStatus::ReleaseUnconfirmed,
                "errno {errno} under {scope:?} must be ReleaseUnconfirmed"
            );
        }
    }
    let custom_err: io::Result<()> = Err(io::Error::other("non-os close failure"));
    assert_eq!(
        classify_close_release_status_for_scope(
            &custom_err,
            CloseScopePrecondition::MacOsUniqueNonGuardedOrdinaryVnode,
        ),
        DescriptorReleaseStatus::ReleaseUnconfirmed
    );

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("eintr_unconfirmed.bin");
    let file = File::create(&path).expect("create");
    let obs = attach_open_description_observer(&path, &file);
    let parked: RefCell<Vec<OwnedFd>> = RefCell::new(Vec::new());
    let mut close_calls = 0usize;

    let err = finish_and_close_file_with(
        file,
        (3, 4),
        Ok(()),
        None,
        FileFinishSyncPolicy::None,
        CloseScopePrecondition::MacOsUniqueNonGuardedOrdinaryVnode,
        File::sync_all,
        |owned_fd, scope| {
            close_calls += 1;
            let fd = owned_fd.as_raw_fd();
            parked.borrow_mut().push(owned_fd);
            let sim_err: io::Result<()> = Err(io::Error::from_raw_os_error(libc::EINTR));
            let status = classify_close_release_status_for_scope(&sim_err, scope);
            (fd, sim_err, status)
        },
    )
    .expect_err("EINTR must fail");

    assert_eq!(close_calls, 1);
    assert!(!err.is_release_confirmed());
    assert_eq!(err.release_status, DescriptorReleaseStatus::ReleaseUnconfirmed);
    assert_eq!(err.close_error.as_ref().and_then(io::Error::raw_os_error), Some(libc::EINTR));
    assert!(is_target_description_open(&obs));
    parked.borrow_mut().clear();
    assert!(!is_target_description_open(&obs));
}

#[test]
fn classify_out_of_scope_eio_conservatively_returns_release_unconfirmed() {
    let eio_res: io::Result<()> = Err(io::Error::from_raw_os_error(libc::EIO));
    assert_eq!(
        classify_close_release_status_for_scope(
            &eio_res,
            CloseScopePrecondition::ConservativeUnverifiedScope,
        ),
        DescriptorReleaseStatus::ReleaseUnconfirmed
    );
    assert_eq!(
        classify_close_release_status_with_target_os(
            false,
            Some(libc::EIO),
            CloseScopePrecondition::MacOsUniqueNonGuardedOrdinaryVnode,
            false,
        ),
        DescriptorReleaseStatus::ReleaseUnconfirmed
    );

    let dir = tempfile::tempdir().expect("tempdir");
    let _inj = ScopedCloseErrnoInjection::install(libc::EIO);

    let file_a = File::create(dir.path().join("unverified_file_eio.bin")).expect("create a");
    let err_a = explicit_finish_and_close_file_production(
        file_a,
        Ok(()),
        None,
        FileFinishSyncPolicy::None,
        CloseScopePrecondition::ConservativeUnverifiedScope,
    )
    .expect_err("conservative file finish EIO must fail");
    assert_eq!(err_a.release_status, DescriptorReleaseStatus::ReleaseUnconfirmed);
    assert!(!err_a.is_release_confirmed());
    assert_eq!(err_a.close_error.as_ref().and_then(io::Error::raw_os_error), Some(libc::EIO));

    let file_b = File::create(dir.path().join("unverified_bw_eio.bin")).expect("create b");
    let mut bw_b = BufWriter::new(file_b);
    bw_b.write_all(b"conservative-scope").expect("write bw_b");
    let err_b = flush_and_close_buf_writer_once(
        bw_b,
        Ok(()),
        FileFinishSyncPolicy::None,
        CloseScopePrecondition::ConservativeUnverifiedScope,
    )
    .expect_err("conservative BufWriter finish EIO must fail");
    assert_eq!(err_b.release_status, DescriptorReleaseStatus::ReleaseUnconfirmed);
    assert!(!err_b.is_release_confirmed());
    assert_eq!(err_b.close_error.as_ref().and_then(io::Error::raw_os_error), Some(libc::EIO));
    assert_eq!(ScopedCloseErrnoInjection::calls(), 2);
}

#[test]
fn buf_writer_flush_and_close_dual_errors_are_both_preserved_without_drop_retry() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("readonly_bufwriter.bin");
    File::create(&path).expect("create target");

    let ro_file = File::open(&path).expect("open read-only");
    let obs = attach_open_description_observer(&path, &ro_file);
    let mut bw = BufWriter::with_capacity(64, ro_file);
    bw.write_all(b"buffered-bytes-that-cannot-flush")
        .expect("buffer in memory");

    let _inj = ScopedCloseErrnoInjection::install(libc::EINTR);
    let err = flush_and_close_buf_writer_once(
        bw,
        Ok(()),
        FileFinishSyncPolicy::SyncAll,
        CloseScopePrecondition::MacOsUniqueNonGuardedOrdinaryVnode,
    )
    .expect_err("flush + close failure must return Err");

    assert_eq!(ScopedCloseErrnoInjection::calls(), 1);
    assert!(!is_target_description_open(&obs));
    assert!(err.userspace_flush_error.is_some());
    assert!(err.sync_error.is_none(), "sync must not run after flush error");
    assert_eq!(err.close_error.as_ref().and_then(io::Error::raw_os_error), Some(libc::EINTR));
    assert_eq!(err.release_status, DescriptorReleaseStatus::ReleaseUnconfirmed);
}

#[test]
fn single_close_ownership_and_sync_panic_unwind_release_open_description() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target_path = dir.path().join("target_slot.bin");
    let file = File::create(&target_path).expect("create target");
    let obs = attach_open_description_observer(&target_path, &file);
    let expected_fd = file.as_raw_fd();
    assert!(is_target_description_open(&obs));

    let (closed_fd, close_res, status) =
        close_owned_file_once(file, CloseScopePrecondition::ConservativeUnverifiedScope);
    assert_eq!(closed_fd, expected_fd);
    assert!(close_res.is_ok());
    assert_eq!(status, DescriptorReleaseStatus::ConfirmedReleased);
    assert!(!is_target_description_open(&obs));

    let panic_path = dir.path().join("sync_panic.bin");
    let panic_file = File::create(&panic_path).expect("create panic_file");
    let panic_obs = attach_open_description_observer(&panic_path, &panic_file);
    assert!(is_target_description_open(&panic_obs));
    let mut close_fn_called_on_panic = false;
    let unwind_res = catch_unwind(AssertUnwindSafe(|| {
        let _ = finish_and_close_file_with(
            panic_file,
            (9, 10),
            Ok(()),
            None,
            FileFinishSyncPolicy::SyncAll,
            CloseScopePrecondition::ConservativeUnverifiedScope,
            |_f| panic!("simulated panic inside pre-close sync"),
            |owned_fd, scope| {
                close_fn_called_on_panic = true;
                close_owned_fd_once(owned_fd, scope)
            },
        );
    }));
    assert!(unwind_res.is_err());
    assert!(!close_fn_called_on_panic);
    assert!(
        !is_target_description_open(&panic_obs),
        "RAII must close open file description on sync unwind"
    );
}

#[test]
fn explicit_finish_proof_never_produced_when_any_prior_flush_sync_or_close_error_occurs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let scope = CloseScopePrecondition::ConservativeUnverifiedScope;

    let p1 = dir.path().join("prior_err.bin");
    let f1 = File::create(&p1).expect("f1");
    let obs1 = attach_open_description_observer(&p1, &f1);
    let prior_err = Err((TTZipStatus::ErrInvalidPassword, "bad password".to_string()));
    let e1 = finish_and_close_file_for_proof(f1, (1, 1), prior_err, None, FileFinishSyncPolicy::None, scope)
        .expect_err("prior error must forbid proof");
    assert!(!is_target_description_open(&obs1));
    assert_eq!(e1.primary_status(), TTZipStatus::ErrInvalidPassword);
    assert_eq!(e1.prior_write_or_encoder_error.as_deref(), Some("bad password"));
    assert!(!e1.has_any_io_finish_error());
    assert!(e1.is_release_confirmed());

    let p2 = dir.path().join("flush_err.bin");
    let f2 = File::create(&p2).expect("f2");
    let obs2 = attach_open_description_observer(&p2, &f2);
    let flush_io_err = Some(io::Error::from_raw_os_error(libc::EIO));
    let e2 = finish_and_close_file_for_proof(f2, (2, 2), Ok(()), flush_io_err, FileFinishSyncPolicy::None, scope)
        .expect_err("flush error must forbid proof");
    assert!(!is_target_description_open(&obs2));
    assert!(e2.userspace_flush_error.is_some());
    assert!(e2.has_any_io_finish_error());

    let p3 = dir.path().join("sync_err.bin");
    let f3 = File::create(&p3).expect("f3");
    let obs3 = attach_open_description_observer(&p3, &f3);
    let e3 = finish_and_close_file_with(
        f3,
        (3, 3),
        Ok(()),
        None,
        FileFinishSyncPolicy::SyncAll,
        scope,
        |_f| Err(io::Error::from_raw_os_error(libc::EIO)),
        close_owned_fd_once,
    )
    .expect_err("sync error must forbid proof");
    assert!(!is_target_description_open(&obs3));
    assert!(e3.sync_error.is_some());
    assert!(e3.close_error.is_none());
    assert!(e3.is_release_confirmed());

    let p4 = dir.path().join("bw_ok.bin");
    let f4 = File::create(&p4).expect("f4");
    let obs4 = attach_open_description_observer(&p4, &f4);
    let fd4 = f4.as_raw_fd();
    let mut bw4 = BufWriter::new(f4);
    bw4.write_all(b"ok").expect("write bw4");
    let closed4 =
        flush_and_close_buf_writer_once(bw4, Ok(()), FileFinishSyncPolicy::None, scope).expect("bw4");
    assert_eq!(closed4, fd4);
    assert!(!is_target_description_open(&obs4));
}
