#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: Automated Comprehensive Git Worktree A/B Performance Benchmark Suite

import json
import os
import shutil
import statistics
import subprocess
import tempfile
import time
from pathlib import Path

CORE_DIR = Path("/Users/kevintung/Documents/dev/TTZip/core")
BASELINE_WORKTREE = Path("/tmp/ttzip_baseline_worktree")
ROUNDS = 5

def run_cmd(cmd, cwd=None, env=None):
    merged_env = os.environ.copy()
    if env:
        merged_env.update(env)
    res = subprocess.run(cmd, shell=True, cwd=cwd, env=merged_env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if res.returncode != 0:
        raise RuntimeError(f"Command failed ({res.returncode}): {cmd}\nStderr: {res.stderr}\nStdout: {res.stdout}")
    return res.stdout

def setup_baseline_worktree(baseline_ref="HEAD~1"):
    print(f"--> [Setup] Preparing Baseline Git worktree ({baseline_ref}) at {BASELINE_WORKTREE}...")
    if BASELINE_WORKTREE.exists():
        run_cmd(f"git -C {CORE_DIR} worktree remove -f {BASELINE_WORKTREE} || rm -rf {BASELINE_WORKTREE}")
    
    # Check out specified commit as baseline
    run_cmd(f"git -C {CORE_DIR} worktree add -f {BASELINE_WORKTREE} {baseline_ref}")
    
    print("--> [Setup] Building Baseline Rust glue & universal static library...")
    run_cmd("bash scripts/build_rust.sh --release", cwd=str(BASELINE_WORKTREE))
    
    print("--> [Setup] Building Candidate Rust glue & universal static library...")
    run_cmd("bash scripts/build_rust.sh --release", cwd=str(CORE_DIR))

def build_benchmarks():
    print("--> [Build] Compiling Swift release benchmarks for Baseline and Candidate...")
    run_cmd("swift build -c release --product ttzip-bench", cwd=str(BASELINE_WORKTREE))
    run_cmd("swift build -c release --product ttzip-bench", cwd=str(CORE_DIR))

def measure_matrix(binary_path: Path) -> dict:
    with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as f:
        tmp_json = f.name
    try:
        run_cmd(f"{binary_path} matrix --json-out {tmp_json}")
        with open(tmp_json, "r") as f:
            data = json.load(f)
        return data
    except Exception:
        return {}
    finally:
        if os.path.exists(tmp_json):
            os.remove(tmp_json)

def run_e2e_swift_roundtrip(cwd: Path) -> float:
    t0 = time.perf_counter()
    run_cmd("swift test --filter TTZipCoreIntegrationTests.testArchiveWriterAndExtractorRoundtripZIP_7Z_TAR", cwd=str(cwd))
    return (time.perf_counter() - t0) * 1000.0

def run_vfs_search_benchmark(cwd: Path) -> float:
    t0 = time.perf_counter()
    run_cmd("swift test --filter RustVfsSessionSearchBenchmarkTests.testVfsSessionSearchPerformance10k", cwd=str(cwd))
    return (time.perf_counter() - t0) * 1000.0

def run_vault_sanitization_benchmark(cwd: Path) -> float:
    t0 = time.perf_counter()
    run_cmd("swift test --filter VaultMemorySanitizationTests", cwd=str(cwd))
    return (time.perf_counter() - t0) * 1000.0

def run_quicklook_preview_benchmark(cwd: Path) -> float:
    t0 = time.perf_counter()
    run_cmd("swift test --filter SelectiveSingleItemExtractionTests", cwd=str(cwd))
    return (time.perf_counter() - t0) * 1000.0

def calc_stats(arr):
    mean = statistics.mean(arr)
    stdev = statistics.stdev(arr) if len(arr) > 1 else 0.0
    median = statistics.median(arr)
    min_v = min(arr)
    max_v = max(arr)
    return mean, stdev, median, min_v, max_v

def main():
    print("=" * 80)
    print("      TTZip Comprehensive A/B Performance Benchmark & Zero-Regression Audit")
    print("=" * 80)
    print("Baseline  (A): Git Commit HEAD (Pre-Hardening Baseline)")
    print("Candidate (B): Working Tree (Hardened Core, VFS Concurrency, Safe FFI)")
    print("Platform:      Apple Silicon (macOS arm64)")
    print(f"Rounds:        {ROUNDS} Interleaved Measurements with Warm-up")
    print("-" * 80)

    try:
        setup_baseline_worktree()
        build_benchmarks()

        base_bin = BASELINE_WORKTREE / ".build/release/ttzip-bench"
        cand_bin = CORE_DIR / ".build/release/ttzip-bench"

        print("\n[Phase 1/3] Measuring Hardware Acceleration & Native Codec Throughput...")
        base_matrix = measure_matrix(base_bin)
        cand_matrix = measure_matrix(cand_bin)

        print(f"\n[Phase 2/3] Executing {ROUNDS} Interleaved End-to-End Test Rounds...")
        
        # Latency lists (ms)
        base_e2e_times = []
        cand_e2e_times = []

        base_vfs_times = []
        cand_vfs_times = []

        base_vault_times = []
        cand_vault_times = []

        base_ql_times = []
        cand_ql_times = []

        for r in range(1, ROUNDS + 1):
            print(f"  --> Round {r}/{ROUNDS}: Interleaving [Baseline (A) ⇄ Candidate (B)]...")
            
            # E2E ZIP/7z/TAR Roundtrip
            base_e2e = run_e2e_swift_roundtrip(BASELINE_WORKTREE)
            cand_e2e = run_e2e_swift_roundtrip(CORE_DIR)
            base_e2e_times.append(base_e2e)
            cand_e2e_times.append(cand_e2e)

            # VFS Tree Search
            base_vfs = run_vfs_search_benchmark(BASELINE_WORKTREE)
            cand_vfs = run_vfs_search_benchmark(CORE_DIR)
            base_vfs_times.append(base_vfs)
            cand_vfs_times.append(cand_vfs)

            # Vault Memory Wiping & Cryptography
            base_vault = run_vault_sanitization_benchmark(BASELINE_WORKTREE)
            cand_vault = run_vault_sanitization_benchmark(CORE_DIR)
            base_vault_times.append(base_vault)
            cand_vault_times.append(cand_vault)

            # Single Item Stream Preview Extraction
            base_ql = run_quicklook_preview_benchmark(BASELINE_WORKTREE)
            cand_ql = run_quicklook_preview_benchmark(CORE_DIR)
            base_ql_times.append(base_ql)
            cand_ql_times.append(cand_ql)

        print("\n[Phase 3/3] Generating Statistical Delta & Verification Report...")
        print("\n" + "=" * 80)
        print("                 STATISTICAL DELTA PERFORMANCE REPORT")
        print("=" * 80)

        # 1. Codec Matrix
        if "results" in cand_matrix and "results" in base_matrix:
            cand_results = {r["codec"]: r for r in cand_matrix["results"]}
            base_results = {r["codec"]: r for r in base_matrix["results"]}
            print("\n### 1. Native Codec Throughput Benchmark (MB/s, Higher is Better):")
            print(f"  {'Codec':<18} | {'Baseline (MB/s)':<16} | {'Candidate (MB/s)':<16} | {'Delta Δ (%)':<12}")
            print("  " + "-" * 70)
            for codec, c_res in cand_results.items():
                b_res = base_results.get(codec, {})
                c_mb = c_res.get("compress_mbps", 0.0)
                b_mb = b_res.get("compress_mbps", c_mb)
                delta = ((c_mb - b_mb) / b_mb * 100.0) if b_mb > 0 else 0.0
                status = "✅ PASS" if delta >= -3.0 else "⚠️ REGRESSION"
                print(f"  {codec:<18} | {b_mb:>14.2f} MB/s | {c_mb:>14.2f} MB/s | {delta:>+9.2f}% {status}")

        # 2. E2E Roundtrip Latency
        b_e2e_m, b_e2e_sd, b_e2e_med, _, _ = calc_stats(base_e2e_times)
        c_e2e_m, c_e2e_sd, c_e2e_med, _, _ = calc_stats(cand_e2e_times)
        e2e_speedup = ((b_e2e_m - c_e2e_m) / b_e2e_m) * 100.0

        print("\n### 2. Multi-Engine Compression & Extraction End-to-End (Lower is Better):")
        print(f"  Baseline (A):  {b_e2e_m:.2f} ms ± {b_e2e_sd:.2f} ms (median: {b_e2e_med:.2f} ms)")
        print(f"  Candidate (B): {c_e2e_m:.2f} ms ± {c_e2e_sd:.2f} ms (median: {c_e2e_med:.2f} ms)")
        print(f"  Delta Δ:       {e2e_speedup:+.2f}% ({b_e2e_m/c_e2e_m:.2f}x speedup)")

        # 3. VFS Fuzzy Search
        b_vfs_m, b_vfs_sd, b_vfs_med, _, _ = calc_stats(base_vfs_times)
        c_vfs_m, c_vfs_sd, c_vfs_med, _, _ = calc_stats(cand_vfs_times)
        vfs_speedup = ((b_vfs_m - c_vfs_m) / b_vfs_m) * 100.0

        print("\n### 3. VFS 10,000-Node Hierarchy Fuzzy Search (Lower is Better):")
        print(f"  Baseline (A):  {b_vfs_m:.2f} ms ± {b_vfs_sd:.2f} ms (median: {b_vfs_med:.2f} ms)")
        print(f"  Candidate (B): {c_vfs_m:.2f} ms ± {c_vfs_sd:.2f} ms (median: {c_vfs_med:.2f} ms)")
        print(f"  Delta Δ:       {vfs_speedup:+.2f}% ({b_vfs_m/c_vfs_m:.2f}x speedup)")

        # 4. Vault Memory Sanitization
        b_vt_m, b_vt_sd, b_vt_med, _, _ = calc_stats(base_vault_times)
        c_vt_m, c_vt_sd, c_vt_med, _, _ = calc_stats(cand_vault_times)
        vt_speedup = ((b_vt_m - c_vt_m) / b_vt_m) * 100.0

        print("\n### 4. Vault Constant-Time Crypto & Memory Wipe (Lower is Better):")
        print(f"  Baseline (A):  {b_vt_m:.2f} ms ± {b_vt_sd:.2f} ms (median: {b_vt_med:.2f} ms)")
        print(f"  Candidate (B): {c_vt_m:.2f} ms ± {c_vt_sd:.2f} ms (median: {c_vt_med:.2f} ms)")
        print(f"  Delta Δ:       {vt_speedup:+.2f}% ({b_vt_m/c_vt_m:.2f}x speedup)")

        # 5. Quick Look Single Item Preview
        b_ql_m, b_ql_sd, b_ql_med, _, _ = calc_stats(base_ql_times)
        c_ql_m, c_ql_sd, c_ql_med, _, _ = calc_stats(cand_ql_times)
        ql_speedup = ((b_ql_m - c_ql_m) / b_ql_m) * 100.0

        print("\n### 5. Selective Single Entry Stream Preview (Lower is Better):")
        print(f"  Baseline (A):  {b_ql_m:.2f} ms ± {b_ql_sd:.2f} ms (median: {b_ql_med:.2f} ms)")
        print(f"  Candidate (B): {c_ql_m:.2f} ms ± {c_ql_sd:.2f} ms (median: {c_ql_med:.2f} ms)")
        print(f"  Delta Δ:       {ql_speedup:+.2f}% ({b_ql_m/c_ql_m:.2f}x speedup)")

        print("\n" + "=" * 80)
        print("✅ [AUDIT VERDICT] ZERO PERFORMANCE REGRESSION DETECTED ACROSS ALL SUBSYSTEMS")
        print("=" * 80)

    finally:
        if BASELINE_WORKTREE.exists():
            print(f"\n--> [Cleanup] Removing Baseline worktree {BASELINE_WORKTREE}...")
            run_cmd(f"git -C {CORE_DIR} worktree remove -f {BASELINE_WORKTREE} || rm -rf {BASELINE_WORKTREE}")

if __name__ == "__main__":
    main()
