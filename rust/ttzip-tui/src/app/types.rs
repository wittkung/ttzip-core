// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Core Application Types and Mode Enums.

/// Active TUI modal and operational mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Explorer,
    Search,
    Preview,
    Progress,
    Help,
    PasswordRecovery,
    RepairWizard,
    ParetoBenchmark,
    SplitManager,
    Exiting,
}

/// TTZip Archive Format enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    SevenZ,
    Unknown,
}

impl ArchiveFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            ArchiveFormat::Zip => "ZIP",
            ArchiveFormat::SevenZ => "7-Zip",
            ArchiveFormat::Unknown => "Unknown",
        }
    }
}


