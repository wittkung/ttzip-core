// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Archive streaming and libarchive adapter module.

use std::sync::OnceLock;
use ttzip_types::scheduler::LocalWorkStealingPool;

pub mod builder;
pub mod in_place_edit;
pub mod mac_metadata;
pub mod nested_vfs;
pub mod repair;
pub mod source;
pub mod split;
pub mod stream_adapter;
pub mod tar;
pub mod ttzip_mt_drainer;
pub mod unified;
pub mod wal_mutation;

pub use builder::*;
pub use in_place_edit::*;
pub use mac_metadata::*;
pub use nested_vfs::*;
pub use repair::*;
pub use source::*;
pub use split::{
    compute_volume_path, detect_volume_chain, MissingVolumeDescriptor, MissingVolumeSeverity,
    PreFlightVolumeProber, SplitVolumeWriter, VirtualMultiVolumeReader, VolumeNamingScheme,
    VolumePartInfo, VolumeSegment,
};
pub use stream_adapter::*;
pub use tar::*;
pub use ttzip_mt_drainer::*;
pub use unified::*;
pub use wal_mutation::*;

/// Returns a cached work-stealing compute pool sized to `num_threads.clamp(1, 64)`.
pub(crate) fn shared_compute_pool(num_threads: usize) -> LocalWorkStealingPool {
    static POOLS: [OnceLock<LocalWorkStealingPool>; 65] = [const { OnceLock::new() }; 65];
    let clamped = num_threads.clamp(1, 64);
    POOLS[clamped]
        .get_or_init(|| LocalWorkStealingPool::with_threads(clamped))
        .clone()
}



