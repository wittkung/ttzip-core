#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

"""Parallel Functional Test Dispatcher for TTZip Rust Microkernel.

Compiles all test binaries in a single cargo invocation, then dispatches
functional/correctness targets across 10 workers at full parallelism.

Performance regression targets (throughput thresholds, cycle-count benchmarks)
are skipped here — they require dedicated CPU isolation and are exclusively
enforced by run_local_ci_gate.sh (Stages 8–37).
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

    # ── 1. Compile all test targets in one shot ──────────────────────────
    t0 = time.time()
    proc = subprocess.Popen(
        [
            "cargo", "test", "-p", "ttzip-engine",
            "--manifest-path", manifest,
            "--no-run", "--message-format=json",
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    all_targets = []
    seen = set()
    for line in proc.stdout:
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
            if msg.get("profile", {}).get("test") is True and msg.get("executable"):
                exe = msg["executable"]
                src_path = msg.get("target", {}).get("src_path", "")
                target_name = msg.get("target", {}).get("name", os.path.basename(exe))
                if "src/lib.rs" in src_path:
                    display_name = "unittests src/lib.rs"
                elif "tests/" in src_path:
                    display_name = "tests/" + os.path.basename(src_path)
                else:
                    display_name = target_name
                if exe not in seen:
                    seen.add(exe)
                    all_targets.append((display_name, exe))
        except Exception:
            pass

    proc.wait()
    if proc.returncode != 0:
        err = proc.stderr.read()
        print(f"Compilation failed with exit code {proc.returncode}:\n{err}", file=sys.stderr)
        sys.exit(1)

    # ── 2. Partition: functional vs performance regression ───────────────
    # These targets contain hardcoded throughput/latency assertions
    # (e.g. MAX_ALLOWED_REGRESSION_PCT = 3.0%) that produce false-positive
    # failures under any CPU contention.  They belong in run_local_ci_gate.sh.
    perf_skip_patterns = (
        "performance_regression",
        "regression_audit",
        "silesia_regression",
        "sevenz_crypto_kdf",
        "benchmark",
    )

    run_targets = []
    skipped = []
    for display_name, exe in all_targets:
        if any(p in display_name for p in perf_skip_patterns):
            skipped.append(display_name)
        else:
            extra_args = []
            # The unit test binary contains a single SHA-256 cycle-count
            # benchmark with a 20 ms hard threshold — skip it here.
            if display_name == "unittests src/lib.rs":
                extra_args = ["--", "--skip", "performance_benchmark"]
            run_targets.append((display_name, exe, extra_args))

    # ── 3. LPT scheduling: longest jobs first to minimize tail idle ──────
    def lpt_key(item):
        name = item[0]
        if "unittests" in name:
            return 0   # ~16 s
        if "sevenz_solid_differential" in name:
            return 1   # ~5 s
        if "corrupted_archive" in name:
            return 2   # ~3 s
        if "adversarial_stress" in name:
            return 3   # ~2 s
        if "paramgrill" in name:
            return 4
        return 10

    run_targets.sort(key=lpt_key)

    # ── 4. Full-parallel dispatch ────────────────────────────────────────
    total = len(run_targets)
    max_workers = min(os.cpu_count() or 8, 10)
    completed_count = 0
    failed = []

    test_env = os.environ.copy()
    test_env["RAYON_NUM_THREADS"] = "2"

    def run_target(target_info):
        display_name, exe, extra_args = target_info
        start = time.time()
        res = subprocess.run(
            [exe] + extra_args,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            text=True, env=test_env,
        )
        dur = time.time() - start
        return display_name, exe, res.returncode, res.stdout, dur

    def handle_result(fut):
        nonlocal completed_count
        completed_count += 1
        display_name, _exe, code, out, dur = fut.result()

        passed = "0"
        for line in out.splitlines():
            if "test result: ok." in line:
                parts = line.split()
                if len(parts) >= 5 and parts[4].startswith("passed"):
                    passed = parts[3]

        if code == 0:
            print(
                f"    ⚡️ [{completed_count:3d}/{total}] "
                f"{display_name:<45s} ok ({passed} passed, {dur:.2f}s)",
                flush=True,
            )
        else:
            print(
                f"    ❌ [{completed_count:3d}/{total}] "
                f"{display_name:<45s} FAILED (exit={code}, {dur:.2f}s)",
                flush=True,
            )
            failed.append((display_name, f"Exit code: {code}\n" + out))

    with ThreadPoolExecutor(max_workers=max_workers) as executor:
        futures = [executor.submit(run_target, t) for t in run_targets]
        for fut in as_completed(futures):
            handle_result(fut)

    total_dur = time.time() - t0

    if skipped:
        print(
            f"\n    ⏭️  Skipped {len(skipped)} performance regression targets "
            f"(enforced by run_local_ci_gate.sh)",
            flush=True,
        )

    if failed:
        print(f"\n{'=' * 70}")
        print(f"❌ {len(failed)}/{total} Test Targets FAILED in {total_dur:.2f}s:")
        for name, out in failed:
            print(f"\n--- {name} Failure Output ---")
            print(out[-2000:])
        sys.exit(1)
    else:
        sys.exit(0)


if __name__ == "__main__":
    main()
