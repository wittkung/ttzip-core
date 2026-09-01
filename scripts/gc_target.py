#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

"""
Micro-surgical Rust target/ directory garbage collector and artifact deduplicator.

Prunes stale hash-suffixed test binaries, historical dSYM bundles, and expired
incremental compilation sessions without invalidating active hot build caches
or triggering SwiftPM/UniFFI C-Bridge rebuild avalanches.
"""

import argparse
import os
import re
import shutil
import sys
import time
from pathlib import Path

# Matches Rust artifact stems: e.g. "libfoo-3a8f1b2c9d0e1f2a.rlib" -> stem: "libfoo", hash: "3a8f1b2c9d0e1f2a"
STEM_PATTERN = re.compile(r"^(.*?)-([0-9a-fA-F]{16})(\..+)?$")


def format_size(bytes_val: int) -> str:
    """Formats byte counts into human-readable strings."""
    val = float(bytes_val)
    for unit in ["B", "KB", "MB", "GB"]:
        if val < 1024.0:
            return f"{val:.1f}{unit}"
        val /= 1024.0
    return f"{val:.1f}TB"


def clean_deps_dir(deps_dir: Path, keep_versions: int = 1, dry_run: bool = False) -> int:
    """
    Cleans stale build artifacts in a deps/ directory by retaining only the
    latest `keep_versions` unique build hashes per crate/test target.
    """
    if not deps_dir.is_dir():
        return 0

    reclaimed_bytes = 0
    # Map: stem -> list of (path, mtime, size, is_dir)
    groups = {}

    try:
        with os.scandir(deps_dir) as it:
            for entry in it:
                m = STEM_PATTERN.match(entry.name)
                if not m:
                    continue
                stem = m.group(1)
                try:
                    stat = entry.stat()
                    groups.setdefault(stem, []).append(
                        (entry.path, stat.st_mtime, stat.st_size, entry.is_dir())
                    )
                except OSError:
                    continue
    except FileNotFoundError:
        return 0

    for stem, items in groups.items():
        # Map: hash -> list of entries with that hash
        seen_hashes = {}
        for path, mtime, size, is_dir in items:
            m = STEM_PATTERN.match(os.path.basename(path))
            h = m.group(2) if m else ""
            seen_hashes.setdefault(h, []).append((path, size, is_dir, mtime))

        # Sort unique hashes by their newest mtime descending
        sorted_unique_hashes = sorted(
            seen_hashes.keys(),
            key=lambda h: max(x[3] for x in seen_hashes[h]),
            reverse=True,
        )

        stale_hashes = sorted_unique_hashes[keep_versions:]
        for sh in stale_hashes:
            for path, size, is_dir, _ in seen_hashes[sh]:
                reclaimed_bytes += size
                if not dry_run:
                    try:
                        if is_dir:
                            shutil.rmtree(path, ignore_errors=True)
                        else:
                            os.remove(path)
                    except OSError:
                        pass

    return reclaimed_bytes


def clean_incremental_dir(inc_dir: Path, max_age_days: int = 7, dry_run: bool = False) -> int:
    """
    Removes incremental compilation sessions older than `max_age_days`.
    """
    if not inc_dir.is_dir():
        return 0

    now = time.time()
    cutoff = now - (max_age_days * 86400)
    reclaimed = 0

    try:
        with os.scandir(inc_dir) as it:
            for entry in it:
                try:
                    stat = entry.stat()
                    if entry.is_dir() and stat.st_mtime < cutoff:
                        reclaimed += 1
                        if not dry_run:
                            shutil.rmtree(entry.path, ignore_errors=True)
                except OSError:
                    continue
    except FileNotFoundError:
        pass

    return reclaimed


def scan_and_prune_target(
    target_dir: Path,
    keep_versions: int = 1,
    inc_days: int = 7,
    dry_run: bool = False,
    quiet: bool = False,
) -> int:
    """
    Scans host and multi-target directories under target_dir and executes pruning.
    """
    if not target_dir.is_dir():
        return 0

    total_reclaimed = 0
    subdirs_to_check = []

    # Host subdirectories
    for mode in ["debug", "release"]:
        subdirs_to_check.append(target_dir / mode)

    # Multi-architecture target subdirectories (e.g., aarch64-apple-darwin, x86_64-apple-darwin)
    try:
        with os.scandir(target_dir) as it:
            for entry in it:
                if entry.is_dir() and ("-apple-" in entry.name or "-linux-" in entry.name):
                    for mode in ["debug", "release"]:
                        subdirs_to_check.append(Path(entry.path) / mode)
    except OSError:
        pass

    for dir_path in subdirs_to_check:
        deps = dir_path / "deps"
        inc = dir_path / "incremental"
        total_reclaimed += clean_deps_dir(deps, keep_versions=keep_versions, dry_run=dry_run)
        total_reclaimed += clean_incremental_dir(inc, max_age_days=inc_days, dry_run=dry_run)

    return total_reclaimed


def main():
    parser = argparse.ArgumentParser(
        description="Micro-surgical GC for TTZip Rust target cache"
    )
    parser.add_argument(
        "--target",
        default="core/rust/target",
        help="Path to target directory (default: core/rust/target)",
    )
    parser.add_argument(
        "--keep",
        type=int,
        default=1,
        help="Number of latest artifact versions to retain per crate (default: 1)",
    )
    parser.add_argument(
        "--inc-days",
        type=int,
        default=7,
        help="Max age in days for incremental compilation sessions (default: 7)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Simulate without deleting files",
    )
    parser.add_argument(
        "--quiet",
        "-q",
        action="store_true",
        help="Suppress informational stdout output",
    )

    args = parser.parse_args()
    target_path = Path(args.target).resolve()

    start_time = time.perf_counter()
    reclaimed = scan_and_prune_target(
        target_path,
        keep_versions=args.keep,
        inc_days=args.inc_days,
        dry_run=args.dry_run,
        quiet=args.quiet,
    )
    elapsed_ms = (time.perf_counter() - start_time) * 1000.0

    if not args.quiet:
        action_str = "Would reclaim" if args.dry_run else "Reclaimed"
        print(
            f"🧹 [TTZip Target GC] {action_str} {format_size(reclaimed)} in {elapsed_ms:.2f}ms. Hot cache intact."
        )


if __name__ == "__main__":
    main()
