// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Nanosecond entry metadata model, sparse extent coalescing, and LinkResolver.

pub mod fields;
pub mod model;
pub mod resolver;
pub mod sparse;
pub mod timestamp;
pub mod types;

pub use fields::EntryFields;
pub use model::TTZipEntry;
pub use resolver::{LinkAction, LinkResolver, LinkResolverStrategy};
pub use sparse::{clean_sparse_extents, coalesce_sparse_extents, SparseExtent};
pub use timestamp::TTZipTimestamp;
pub use types::TTZipFileType;
