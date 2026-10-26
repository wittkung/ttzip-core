// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Typestate extraction session protocol.

use std::marker::PhantomData;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;

use crate::fs::collision::commit::DFSCommitWalker;
use crate::fs::collision::fs_ops::FsOps;
use crate::fs::collision::preflight::{post_stage_detect, preflight_detect};
use crate::fs::collision::staging::StagingArea;
use crate::fs::collision::table::ConflictTable;
use crate::fs::collision::{
    CollisionError, ExtractionOutcome, ItemMeta, OutcomeStatus,
};
pub use crate::fs::collision::ResolutionSet;

pub struct Created;
pub struct Preflighted {
    pub table: ConflictTable,
}
pub struct Staged {
    pub staging: StagingArea,
    pub table: Option<ConflictTable>,
}
pub struct Committed {
    pub outcome: ExtractionOutcome,
}
pub struct Aborted {
    pub outcome: ExtractionOutcome,
}

pub struct ExtractionSession<State> {
    pub destination_root: PathBuf,
    pub state_data: State,
    _marker: PhantomData<State>,
}

pub type Session<State> = ExtractionSession<State>;

impl ExtractionSession<Created> {
    pub fn new(destination_root: impl Into<PathBuf>) -> Self {
        Self {
            destination_root: destination_root.into(),
            state_data: Created,
            _marker: PhantomData,
        }
    }

    pub fn preflight<F: FsOps>(
        self,
        fs: &F,
        sorted_items: &[(PathBuf, ItemMeta)],
    ) -> Result<ExtractionSession<Preflighted>, CollisionError> {
        let dest_c = match std::ffi::CString::new(self.destination_root.as_os_str().as_bytes()) {
            Ok(c) => c,
            Err(_) => return Err(CollisionError::DestinationUnavailable(libc::EINVAL)),
        };

        // SAFETY: open destination directory with O_RDONLY | O_DIRECTORY | O_CLOEXEC
        let dirfd = unsafe { libc::open(dest_c.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) };
        let table = if dirfd < 0 {
            ConflictTable::builder().build()?
        } else {
            struct FdGuard(libc::c_int);
            impl Drop for FdGuard {
                fn drop(&mut self) {
                    // SAFETY: closing open fd
                    unsafe { libc::close(self.0); }
                }
            }
            let _guard = FdGuard(dirfd);
            preflight_detect(fs, dirfd, &self.destination_root, sorted_items)?
        };

        Ok(ExtractionSession {
            destination_root: self.destination_root,
            state_data: Preflighted { table },
            _marker: PhantomData,
        })
    }

    pub fn stage(self) -> Result<ExtractionSession<Staged>, std::io::Error> {
        let staging = StagingArea::create(&self.destination_root)?;
        Ok(ExtractionSession {
            destination_root: self.destination_root,
            state_data: Staged {
                staging,
                table: None,
            },
            _marker: PhantomData,
        })
    }

    pub fn stage_with(self, staging: StagingArea) -> ExtractionSession<Staged> {
        ExtractionSession {
            destination_root: self.destination_root,
            state_data: Staged {
                staging,
                table: None,
            },
            _marker: PhantomData,
        }
    }

    pub fn abort(self) -> (ExtractionSession<Aborted>, ExtractionOutcome) {
        let outcome = ExtractionOutcome {
            status: OutcomeStatus::Stopped,
            ..Default::default()
        };
        (
            ExtractionSession {
                destination_root: self.destination_root,
                state_data: Aborted { outcome: outcome.clone() },
                _marker: PhantomData,
            },
            outcome,
        )
    }
}

impl ExtractionSession<Preflighted> {
    pub fn table(&self) -> &ConflictTable {
        &self.state_data.table
    }

    pub fn stage(self) -> Result<ExtractionSession<Staged>, std::io::Error> {
        let staging = StagingArea::create(&self.destination_root)?;
        Ok(ExtractionSession {
            destination_root: self.destination_root,
            state_data: Staged {
                staging,
                table: Some(self.state_data.table),
            },
            _marker: PhantomData,
        })
    }

    pub fn stage_with(self, staging: StagingArea) -> ExtractionSession<Staged> {
        ExtractionSession {
            destination_root: self.destination_root,
            state_data: Staged {
                staging,
                table: Some(self.state_data.table),
            },
            _marker: PhantomData,
        }
    }

    pub fn abort(self) -> (ExtractionSession<Aborted>, ExtractionOutcome) {
        let outcome = ExtractionOutcome {
            status: OutcomeStatus::Stopped,
            ..Default::default()
        };
        (
            ExtractionSession {
                destination_root: self.destination_root,
                state_data: Aborted { outcome: outcome.clone() },
                _marker: PhantomData,
            },
            outcome,
        )
    }
}

impl ExtractionSession<Staged> {
    pub fn staging(&self) -> &StagingArea {
        &self.state_data.staging
    }

    pub fn table(&self) -> Option<&ConflictTable> {
        self.state_data.table.as_ref()
    }

    pub fn post_stage_detect<F: FsOps>(&mut self, fs: &F) -> Result<&ConflictTable, CollisionError> {
        let dest_c = match std::ffi::CString::new(self.destination_root.as_os_str().as_bytes()) {
            Ok(c) => c,
            Err(_) => return Err(CollisionError::DestinationUnavailable(libc::EINVAL)),
        };

        // SAFETY: open destination directory
        let dirfd = unsafe { libc::open(dest_c.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) };
        if dirfd < 0 {
            let table = ConflictTable::builder().build()?;
            self.state_data.table = Some(table);
        } else {
            struct FdGuard(libc::c_int);
            impl Drop for FdGuard {
                fn drop(&mut self) {
                    // SAFETY: closing open fd
                    unsafe { libc::close(self.0); }
                }
            }
            let _guard = FdGuard(dirfd);
            let payload_path = self.state_data.staging.payload_dir().0;
            let table = post_stage_detect(fs, dirfd, &payload_path)?;
            self.state_data.table = Some(table);
        }

        Ok(self.state_data.table.as_ref().unwrap())
    }

    pub fn commit<F: FsOps>(
        self,
        fs: &F,
        resolutions: &ResolutionSet,
    ) -> Result<(ExtractionSession<Committed>, ExtractionOutcome), std::io::Error> {
        let empty_table;
        let table = match &self.state_data.table {
            Some(t) => t,
            None => {
                empty_table = ConflictTable::builder().build().map_err(|e| {
                    std::io::Error::other(e.to_string())
                })?;
                &empty_table
            }
        };

        resolutions.validate(table).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string())
        })?;

        let dest_dir = fs.openat_dir_nofollow(libc::AT_FDCWD, &self.destination_root)?;
        let payload_path = self.state_data.staging.payload_dir().0;
        let payload_dir = fs.openat_dir_nofollow(libc::AT_FDCWD, &payload_path)?;
        let displaced_path = self.state_data.staging.displaced_dir();
        let displaced_dir = fs.openat_dir_nofollow(libc::AT_FDCWD, &displaced_path)?;
        let caps = fs.probe_vol_caps(&self.destination_root)?;

        let walker = DFSCommitWalker::new(
            fs,
            dest_dir.as_raw_fd(),
            payload_dir.as_raw_fd(),
            displaced_dir.as_raw_fd(),
            caps,
        );

        let (mut outcome, committed_mapping) = walker.commit_with_mapping(table, resolutions)?;
        crate::fs::staged_metadata::apply_committed_metadata(
            &payload_path,
            &self.destination_root,
            &committed_mapping,
            &mut outcome,
        );

        let has_placed_items = (outcome.extracted
            + outcome.replaced
            + outcome.kept_both
            + outcome.merged_dirs)
            > 0;
        if outcome.status == OutcomeStatus::Completed || has_placed_items {
            self.state_data.staging.commit();
        }


        Ok((
            ExtractionSession {
                destination_root: self.destination_root,
                state_data: Committed { outcome: outcome.clone() },
                _marker: PhantomData,
            },
            outcome,
        ))
    }

    /// Commits staged items while enforcing `policy` as the fallback for any conflict in `table`
    /// not explicitly covered by `resolutions`.
    ///
    /// - When `policy == CollisionPolicy::Stop` and `table` contains any conflict not covered by
    ///   `resolutions.overrides` or `resolutions.default_action`, aborts staging before moving any
    ///   files and returns `OutcomeStatus::Stopped`.
    /// - When `policy == CollisionPolicy::Ask` and `table` contains any uncovered conflict,
    ///   returns `Err(InvalidInput)` without touching the destination.
    /// - When `policy` is `KeepBoth`, `Replace`, or `Skip` and `resolutions.default_action` is
    ///   `None`, supplies `policy`'s action as `default_action`.
    pub fn commit_with_policy<F: FsOps>(
        mut self,
        fs: &F,
        policy: crate::fs::collision::CollisionPolicy,
        resolutions: &ResolutionSet,
    ) -> Result<ExtractionOutcome, std::io::Error> {
        if self.state_data.table.is_none() {
            self.post_stage_detect(fs).map_err(|e| {
                std::io::Error::other(e.to_string())
            })?;
        }
        let table = self.state_data.table.as_ref().expect("table populated");
        resolutions.validate(table).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string())
        })?;

        let all_covered = table.is_empty()
            || resolutions.default_action.is_some()
            || resolutions.overrides.len() == table.len();

        if !all_covered {
            match policy {
                crate::fs::collision::CollisionPolicy::Stop => {
                    let (_aborted, outcome) = self.abort();
                    return Ok(outcome);
                }
                crate::fs::collision::CollisionPolicy::Ask => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "invalid resolution: unresolved conflicts remain without default_action or override",
                    ));
                }
                _ => {}
            }
        }

        let mut effective = resolutions.clone();
        if effective.default_action.is_none() {
            effective.default_action = policy.to_resolution_set().and_then(|r| r.default_action);
        }
        let (_committed, outcome) = self.commit(fs, &effective)?;
        Ok(outcome)
    }

    pub fn abort(self) -> (ExtractionSession<Aborted>, ExtractionOutcome) {
        self.state_data.staging.abort();
        let outcome = ExtractionOutcome {
            status: OutcomeStatus::Stopped,
            ..Default::default()
        };
        (
            ExtractionSession {
                destination_root: self.destination_root,
                state_data: Aborted { outcome: outcome.clone() },
                _marker: PhantomData,
            },
            outcome,
        )
    }
}
