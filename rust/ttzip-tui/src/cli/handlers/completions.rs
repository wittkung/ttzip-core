// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Dynamic shell completion generator for Bash, Zsh, Fish, PowerShell, and Elvish.

use crate::cli::args::Cli;
use clap::CommandFactory;
use clap_complete::{generate, Shell};
use std::io;

/// Executes `completions` subcommand by generating dynamic shell completions.
pub fn execute_completions(shell: &str) -> Result<(), String> {
    let shell_type = match shell.to_lowercase().as_str() {
        "bash" => Shell::Bash,
        "zsh" => Shell::Zsh,
        "fish" => Shell::Fish,
        "powershell" | "pwsh" => Shell::PowerShell,
        "elvish" => Shell::Elvish,
        other => {
            return Err(format!(
                "Unsupported shell '{}'. Supported shells: bash, zsh, fish, powershell, elvish",
                other
            ))
        }
    };

    let mut cmd = Cli::command();
    generate(shell_type, &mut cmd, "ttzip", &mut io::stdout());
    Ok(())
}
