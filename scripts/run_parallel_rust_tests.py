#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

"""High-Performance Parallel Test Dispatcher for TTZip Rust Microkernel Suites.

Compiles all 275+ test binaries concurrently in a single cargo build invocation,
then dispatches test binaries in parallel across physical CPU cores to eliminate
OS process cold-start and sequential cargo-evaluation bottlenecks.
"""

import json
import os
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed

def main():
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    manifest = os.path.join(root, "rust", "ttzip-engine", "Cargo.toml")
    
    # 1. Compile all test targets concurrently
    t0 = time.time()
    proc = subprocess.Popen(
        ["cargo", "test", "-p", "ttzip-engine", "--manifest-path", manifest, "--no-run", "--message-format=json"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )
    
    targets = []
    seen = set()
    for line in proc.stdout:
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
            if msg.get("profile", {}).get("test") is True and msg.get("executable"):
                exe = msg["executable"]
                target_name = msg.get("target", {}).get("name", os.path.basename(exe))
                src_path = msg.get("target", {}).get("src_path", "")
                if "src/lib.rs" in src_path:
                    display_name = "unittests src/lib.rs"
                elif "tests/" in src_path:
                    rel = "tests/" + os.path.basename(src_path)
                    display_name = rel
                else:
                    display_name = target_name
                if exe not in seen:
                    seen.add(exe)
                    targets.append((display_name, exe))
        except Exception:
            pass
            
    proc.wait()
    if proc.returncode != 0:
        err = proc.stderr.read()
        print(f"Compilation failed with exit code {proc.returncode}:\n{err}", file=sys.stderr)
        sys.exit(1)
        
    total = len(targets)
    
    # Partition targets into:
    # 1. Parallel targets (functional, compliance, fuzz, security, and unittests)
    # 2. Performance regression targets (*performance_regression_tests.rs) executed in isolation
    #    to prevent CPU cache & memory bus contention from violating Invariant 6 (<= 3.0% regression limit).
    parallel_targets = []
    performance_targets = []
    for item in targets:
        name = item[0]
        if "performance_regression" in name:
            performance_targets.append(item)
        else:
            parallel_targets.append(item)

    max_workers = min(os.cpu_count() or 8, 12)
    completed_count = 0
    failed = []
    
    def run_target(target_info):
        display_name, exe = target_info
        start = time.time()
        res = subprocess.run([exe], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        dur = time.time() - start
        return display_name, exe, res.returncode, res.stdout, dur

    def handle_result(fut):
        nonlocal completed_count
        completed_count += 1
        display_name, exe, code, out, dur = fut.result()
        
        passed = "0"
        for line in out.splitlines():
            if "test result: ok." in line:
                parts = line.split()
                if len(parts) >= 5 and parts[4].startswith("passed"):
                    passed = parts[3]
                    
        if code == 0:
            print(f"    ⚡️ [{completed_count:3d}/{total}] {display_name:<45s} ok ({passed} passed, {dur:.2f}s)", flush=True)
        else:
            print(f"    ❌ [{completed_count:3d}/{total}] {display_name:<45s} FAILED ({dur:.2f}s)", flush=True)
            failed.append((display_name, out))

    # Phase 1: Execute general functional & unit tests with full core concurrency
    with ThreadPoolExecutor(max_workers=max_workers) as executor:
        futures = [executor.submit(run_target, t) for t in parallel_targets]
        for fut in as_completed(futures):
            handle_result(fut)

    # Phase 2: Execute performance regression tests in quiet isolation (single worker)
    # to guarantee zero noisy-neighbor CPU cache evictions and strict Invariant 6 compliance.
    with ThreadPoolExecutor(max_workers=1) as executor:
        futures = [executor.submit(run_target, t) for t in performance_targets]
        for fut in futures:
            handle_result(fut)

    total_dur = time.time() - t0
    if failed:
        print(f"\n======================================================================")
        print(f"❌ {len(failed)}/{total} Test Targets FAILED in {total_dur:.2f}s:")
        for name, out in failed:
            print(f"\n--- {name} Failure Output ---")
            print(out[-2000:])
        sys.exit(1)
    else:
        sys.exit(0)

if __name__ == "__main__":
    main()
