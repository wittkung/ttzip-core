// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Hardlink resolution state machine supporting Tar and NewCpio strategies.

use std::collections::HashMap;

use super::model::TTZipEntry;
use super::types::TTZipFileType;

/// Strategy for resolving hardlinks across multi-entry archive pipelines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LinkResolverStrategy {
    /// Standard Tar/PAX format: First node contains physical payload; subsequent nodes are zero-size pointers.
    #[default]
    Tar,
    /// SVR4 / New ASCII CPIO: Initial nodes contain metadata only; final node contains payload.
    NewCpio,
}

/// Action determined by the `LinkResolver` for a given entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkAction {
    /// Not a hardlink or single link; write metadata and data normally.
    Normal,
    /// Tar first occurrence: write entry header and full payload data.
    TarFirstEntry { write_data: bool },
    /// Tar subsequent occurrence: write entry header pointing to canonical path, zero data bytes.
    TarHardlink { target: String, write_data: bool },
    /// NewCpio intermediate occurrence: write header metadata with size 0, no data bytes.
    CpioMetadataOnly { write_data: bool },
    /// NewCpio final occurrence: write full payload data and actual size.
    CpioFinalData { write_data: bool },
}

#[derive(Debug, Clone)]
struct InodeLinkState {
    canonical_path: String,
    total_links: u64,
    remaining_links: u64,
    original_size: u64,
    seen_count: u64,
}

/// High-performance memory-safe hardlink resolution state machine.
#[derive(Debug, Clone, Default)]
pub struct LinkResolver {
    strategy: LinkResolverStrategy,
    inodes: HashMap<(u64, u64), InodeLinkState>,
}

impl LinkResolver {
    /// Creates a new `LinkResolver` with the specified strategy.
    pub fn new(strategy: LinkResolverStrategy) -> Self {
        Self {
            strategy,
            inodes: HashMap::new(),
        }
    }

    /// Strategy currently in effect.
    #[inline]
    pub const fn strategy(&self) -> LinkResolverStrategy {
        self.strategy
    }

    /// Clears all tracked inode states.
    pub fn reset(&mut self) {
        self.inodes.clear();
    }

    /// Number of inodes currently being tracked awaiting link resolution.
    #[inline]
    pub fn pending_inodes_count(&self) -> usize {
        self.inodes.len()
    }

    /// Resolves an entry against the hardlink state machine and updates its attributes in-place.
    pub fn resolve(&mut self, entry: &mut TTZipEntry) -> LinkAction {
        if entry.ino == 0 || entry.nlink <= 1 || entry.file_type == TTZipFileType::Directory {
            return LinkAction::Normal;
        }

        let key = (entry.dev, entry.ino);

        match self.strategy {
            LinkResolverStrategy::Tar => {
                if let Some(state) = self.inodes.get_mut(&key) {
                    state.seen_count = state.seen_count.saturating_add(1);
                    state.remaining_links = state.remaining_links.saturating_sub(1);
                    let target = state.canonical_path.clone();

                    entry.set_hardlink(&target);
                    entry.set_size(0);

                    if state.remaining_links == 0 {
                        self.inodes.remove(&key);
                    }

                    LinkAction::TarHardlink {
                        target,
                        write_data: false,
                    }
                } else {
                    let total_links = entry.nlink;
                    let remaining_links = total_links.saturating_sub(1);
                    if remaining_links > 0 {
                        self.inodes.insert(
                            key,
                            InodeLinkState {
                                canonical_path: entry.pathname.clone(),
                                total_links,
                                remaining_links,
                                original_size: entry.size,
                                seen_count: 1,
                            },
                        );
                    }
                    LinkAction::TarFirstEntry { write_data: true }
                }
            }
            LinkResolverStrategy::NewCpio => {
                if let Some(state) = self.inodes.get_mut(&key) {
                    state.seen_count = state.seen_count.saturating_add(1);
                    let is_final =
                        state.remaining_links <= 1 || state.seen_count >= state.total_links;
                    state.remaining_links = state.remaining_links.saturating_sub(1);

                    if is_final {
                        entry.set_size(state.original_size);
                        self.inodes.remove(&key);
                        LinkAction::CpioFinalData { write_data: true }
                    } else {
                        entry.set_size(0);
                        LinkAction::CpioMetadataOnly { write_data: false }
                    }
                } else {
                    let total_links = entry.nlink;
                    let remaining_links = total_links.saturating_sub(1);
                    let original_size = entry.size;

                    if remaining_links > 0 {
                        self.inodes.insert(
                            key,
                            InodeLinkState {
                                canonical_path: entry.pathname.clone(),
                                total_links,
                                remaining_links,
                                original_size,
                                seen_count: 1,
                            },
                        );
                        entry.set_size(0);
                        LinkAction::CpioMetadataOnly { write_data: false }
                    } else {
                        LinkAction::Normal
                    }
                }
            }
        }
    }
}
