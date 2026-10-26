// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Standalone CLI Engine and Interactive TUI Runner for TTZip.

pub mod args;
pub mod braille_plotter;
pub mod format;
pub mod handlers;
pub mod tui_runner;

pub use args::*;
pub use braille_plotter::*;
pub use format::*;
pub use handlers::*;
pub use tui_runner::*;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_integration;
