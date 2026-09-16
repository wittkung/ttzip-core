// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Archive streaming and libarchive adapter module.

pub mod builder;
pub mod five_layer_state_machine;
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
pub mod zero_vtable_dispatch;

pub use builder::*;
pub use five_layer_state_machine::*;
pub use in_place_edit::*;
pub use mac_metadata::*;
pub use nested_vfs::*;
pub use repair::*;
pub use source::*;
pub use split::{
    compute_volume_path, detect_volume_chain, SplitVolumeWriter, VirtualMultiVolumeReader,
    VolumeNamingScheme, VolumeSegment,
};
pub use stream_adapter::*;
pub use tar::*;
pub use ttzip_mt_drainer::*;
pub use unified::*;
pub use wal_mutation::*;
pub use zero_vtable_dispatch::*;



