// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Shared test fixtures and synthetic corpus generation utilities.

pub mod datagen;

#[allow(unused_imports, reason = "Re-export datagen symbols for integration tests that include fixtures module")]
pub use datagen::*;
