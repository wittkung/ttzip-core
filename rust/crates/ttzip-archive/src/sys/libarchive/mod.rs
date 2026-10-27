// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Safe libarchive RAII handles and raw C-ABI bindings.

pub mod guards;
pub mod sys;

pub use guards::*;
pub use sys::*;
