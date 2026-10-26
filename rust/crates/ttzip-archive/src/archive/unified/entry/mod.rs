// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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
