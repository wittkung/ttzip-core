// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Subcommand execution handler for benchmark and Pareto plotting.

use crate::cli::braille_plotter::run_cli_benchmark;

/// Executes headless `bench` subcommand.
pub fn execute_bench(
    mips: bool,
    pareto: bool,
    threads: u32,
    dict_mb: u32,
    iterations: u32,
) -> Result<(), String> {
    run_cli_benchmark(mips, pareto, threads, dict_mb, iterations)
}
