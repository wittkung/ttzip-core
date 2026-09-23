// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Write-Ahead-Log (WAL) transaction journal for atomic in-place archive mutations.

use crate::types::TTZipStatus;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub static INPLACE_COUNTER: AtomicU64 = AtomicU64::new(1);

/// State of Write-Ahead-Log transaction journal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WalState {
    Initiated,
    ShadowWritten,
    Committed,
    Aborted,
}

/// Write-Ahead-Log persistent metadata journal record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalJournalRecord {
    pub session_id: u64,
    pub timestamp_epoch_secs: u64,
    pub archive_path: String,
    pub shadow_path: String,
    pub state: WalState,
    pub action_count: usize,
}

/// WAL journal handle providing crash resilience and transactional rollback.
pub struct WalTransactionJournal {
    pub path: PathBuf,
}

impl WalTransactionJournal {
    /// Initializes and flushes an active WAL journal record to disk.
    pub fn begin(wal_path: &Path, archive_path: &Path, shadow_path: &Path, action_count: usize) -> Result<Self, TTZipStatus> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let record = WalJournalRecord {
            session_id: INPLACE_COUNTER.load(Ordering::Relaxed),
            timestamp_epoch_secs: timestamp,
            archive_path: archive_path.to_string_lossy().to_string(),
            shadow_path: shadow_path.to_string_lossy().to_string(),
            state: WalState::Initiated,
            action_count,
        };
        let data = serde_json::to_vec_pretty(&record).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        fs::write(wal_path, data).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        Ok(Self { path: wal_path.to_path_buf() })
    }

    /// Marks shadow payload as successfully written to disk.
    pub fn mark_shadow_written(&self) -> Result<(), TTZipStatus> {
        self.update_state(WalState::ShadowWritten)
    }

    /// Marks the transaction as finalized and committed.
    pub fn mark_committed(&self) -> Result<(), TTZipStatus> {
        self.update_state(WalState::Committed)
    }

    fn update_state(&self, state: WalState) -> Result<(), TTZipStatus> {
        let content = fs::read_to_string(&self.path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let mut record = serde_json::from_str::<WalJournalRecord>(&content).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
        record.state = state;
        let data = serde_json::to_vec_pretty(&record).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
        fs::write(&self.path, data).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        Ok(())
    }

    /// Removes the WAL journal file from the filesystem.
    pub fn cleanup(&self) {
        if self.path.exists() {
            let _ = fs::remove_file(&self.path);
        }
    }
}
