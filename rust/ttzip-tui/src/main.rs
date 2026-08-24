// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! TTZip Standalone Native CLI & Interactive TUI Application Entry Point.

use clap::{CommandFactory, Parser};
use std::io::IsTerminal;
use ttzip_tui::cli::{
    execute_bench, execute_cat, execute_check, execute_comment, execute_completions,
    execute_convert, execute_create, execute_delete, execute_diff, execute_doctor, execute_extract,
    execute_hash, execute_info, execute_join, execute_list, execute_lock, execute_recover,
    execute_repair, execute_split, execute_tree, execute_update, run_interactive_tui, Cli, Commands,
};

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Some(Commands::List {
            archive,
            password,
            json,
            include,
            exclude,
        }) => execute_list(&archive, password.as_deref(), json, &include, &exclude).map_err(|e| e.into()),
        Some(Commands::Extract {
            archive,
            output,
            password,
            threads,
            verbose,
            dry_run,
            include,
            exclude,
        }) => execute_extract(
            &archive,
            output.as_deref(),
            password.as_deref(),
            threads,
            verbose,
            dry_run,
            &include,
            &exclude,
        )
        .map_err(|e| e.into()),
        Some(Commands::Create {
            archive,
            sources,
            format,
            level,
            password,
            threads,
            volume_size,
            dry_run: _,
            include: _,
            exclude: _,
        }) => execute_create(
            &archive,
            &sources,
            format.as_deref(),
            level,
            password.as_deref(),
            threads,
            volume_size.as_deref(),
        )
        .map_err(|e| e.into()),
        Some(Commands::Cat {
            archive,
            entry_path,
            password,
        }) => execute_cat(&archive, &entry_path, password.as_deref()).map_err(|e| e.into()),
        Some(Commands::Check {
            archive,
            password,
            deep,
            json,
        }) => execute_check(&archive, password.as_deref(), deep, json).map_err(|e| e.into()),
        Some(Commands::Comment {
            archive,
            comment,
            json,
        }) => execute_comment(&archive, comment.as_deref(), json).map_err(|e| e.into()),
        Some(Commands::Convert {
            source_archive,
            destination_archive,
            format,
            level,
        }) => execute_convert(
            &source_archive,
            &destination_archive,
            format.as_deref(),
            level,
        )
        .map_err(|e| e.into()),
        Some(Commands::Delete {
            archive,
            entries,
            json,
        }) => execute_delete(&archive, &entries, json).map_err(|e| e.into()),
        Some(Commands::Diff {
            archive_a,
            archive_b,
            json,
        }) => execute_diff(&archive_a, &archive_b, json).map_err(|e| e.into()),
        Some(Commands::Hash {
            path,
            algorithm,
            json,
        }) => execute_hash(&path, &algorithm, json).map_err(|e| e.into()),
        Some(Commands::Info { archive, json }) => {
            execute_info(&archive, json).map_err(|e| e.into())
        }
        Some(Commands::Lock {
            archive,
            unlock,
            json,
        }) => execute_lock(&archive, unlock, json).map_err(|e| e.into()),
        Some(Commands::Tree {
            archive,
            depth,
            include,
            exclude,
            json,
        }) => execute_tree(&archive, depth, json, &include, &exclude).map_err(|e| e.into()),
        Some(Commands::Update {
            archive,
            sources,
            level,
            json,
        }) => execute_update(&archive, &sources, level, json).map_err(|e| e.into()),
        Some(Commands::Doctor { json }) => execute_doctor(json).map_err(|e| e.into()),
        Some(Commands::Completions { shell }) => execute_completions(&shell).map_err(|e| e.into()),
        Some(Commands::Recover {
            archive,
            dictionary,
            threads,
            json,
        }) => execute_recover(&archive, &dictionary, threads, json).map_err(|e| e.into()),
        Some(Commands::Repair {
            damaged_archive,
            output,
            format,
            json,
        }) => execute_repair(&damaged_archive, &output, format.as_deref(), json).map_err(|e| e.into()),
        Some(Commands::Split {
            source_archive,
            volume_size,
            output_dir,
            naming,
        }) => execute_split(
            &source_archive,
            &volume_size,
            output_dir.as_deref(),
            naming.as_deref(),
        )
        .map_err(|e| e.into()),
        Some(Commands::Join {
            first_volume,
            output,
            json,
        }) => execute_join(&first_volume, &output, json).map_err(|e| e.into()),
        Some(Commands::Bench {
            mips,
            pareto,
            threads,
            dict_mb,
            iterations,
        }) => execute_bench(mips, pareto, threads, dict_mb, iterations).map_err(|e| e.into()),
        None => {
            if let Some(archive_path) = cli.archive {
                if !archive_path.exists() {
                    eprintln!("[ERROR] Target archive does not exist: {}", archive_path.display());
                    std::process::exit(1);
                }
                // TTY Intelligence: Fall back to non-interactive listing if stdout is not a terminal
                if std::io::stdout().is_terminal() {
                    run_interactive_tui(archive_path)
                } else {
                    execute_list(&archive_path, None, false, &[], &[]).map_err(|e| e.into())
                }
            } else {
                let mut cmd = Cli::command();
                let _ = cmd.print_help();
                println!();
                Ok(())
            }
        }
    };

    if let Err(err) = result {
        eprintln!("[ERROR] {}", err);
        std::process::exit(1);
    }
}
