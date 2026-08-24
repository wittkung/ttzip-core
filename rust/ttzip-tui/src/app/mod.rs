// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Core Application State Machine, Key Action Dispatch, and Safe Extraction Integration.

pub mod extract;
pub mod input;
pub mod modal_state;
pub mod preview;
pub mod recovery_runner;
pub mod repair_runner;
pub mod split;
pub mod state;
pub mod types;

pub use modal_state::*;
pub use recovery_runner::*;
pub use repair_runner::*;
pub use state::*;
pub use types::*;

#[cfg(test)]
mod tests;
