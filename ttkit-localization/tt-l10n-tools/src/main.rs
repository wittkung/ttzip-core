// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTKit: High-performance cross-platform system utility foundation.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

pub mod codegen;
pub mod fuzzer;
pub mod validator;

#[derive(Parser)]
#[command(name = "tt-l10n-tools")]
#[command(about = "TTKit Localization Tooling & CI Quality Governance")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate Rust static slice files from JSON contract
    GenerateRust {
        #[arg(short, long)]
        contract: PathBuf,
        #[arg(short, long)]
        out_dir: PathBuf,
    },
    /// Generate Swift enum files from JSON contract
    GenerateSwift {
        #[arg(short, long)]
        contract: PathBuf,
        #[arg(short, long)]
        out_file: PathBuf,
    },
    /// Validate 1:1 key parity across all languages
    ValidateParity {
        #[arg(short, long)]
        contract: PathBuf,
    },
    /// Validate anti-fake translation duplicate ratio
    ValidateAntiFake {
        #[arg(short, long)]
        contract: PathBuf,
        #[arg(short, long, default_value_t = 0.15)]
        threshold: f64,
    },
    /// Validate format specifier consistency
    ValidateFormatSpecifiers {
        #[arg(short, long)]
        contract: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::GenerateRust { contract, out_dir } => {
            codegen::generate_rust_catalogs(&contract, &out_dir)?;
            println!("✓ Rust catalogs generated successfully in {:?}", out_dir);
        }
        Commands::GenerateSwift { contract, out_file } => {
            codegen::generate_swift_enums(&contract, &out_file)?;
            println!("✓ Swift enums generated successfully in {:?}", out_file);
        }
        Commands::ValidateParity { contract } => {
            validator::validate_parity(&contract)?;
        }
        Commands::ValidateAntiFake { contract, threshold } => {
            validator::validate_anti_fake(&contract, threshold)?;
        }
        Commands::ValidateFormatSpecifiers { contract } => {
            fuzzer::validate_format_specifiers(&contract)?;
        }
    }

    Ok(())
}
