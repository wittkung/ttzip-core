#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

"""
TTZip Single-File Line Count (LOC) Defense Gate
Scans Sources/ and rust/ to enforce Single Responsibility Principle (SRP)
and block monolithic god-files exceeding the 800 LOC hard threshold.
"""

import sys
import os
from pathlib import Path

MAX_LOC_THRESHOLD = 800
SOURCE_DIRS = ["Sources", "rust"]
SOURCE_EXTENSIONS = {".swift", ".rs", ".c", ".h", ".cpp", ".hpp", ".m"}
IGNORED_DIRS = {"target", "target_sdk", ".build", "Vendor", ".git", "DerivedData", "Generated", "mpv"}
IGNORED_FILENAMES = {"ttzip_engineFFI.h"}

# Terminal colors
C_RESET = "\033[0m"
C_BOLD = "\033[1m"
C_RED = "\033[1;31m"
C_GREEN = "\033[1;32m"
C_YELLOW = "\033[1;33m"
C_CYAN = "\033[1;36m"

def get_repo_root() -> Path:
    return Path(__file__).resolve().parent.parent

def scan_loc_gate(root_dir: Path, max_loc: int = MAX_LOC_THRESHOLD):
    violations = []
    scanned_files = 0
    total_loc = 0

    for sdir in SOURCE_DIRS:
        dir_path = root_dir / sdir
        if not dir_path.exists():
            continue

        for root, dirs, files in os.walk(dir_path):
            # Prune ignored directories in-place
            dirs[:] = [
                d for d in dirs
                if d not in IGNORED_DIRS and not d.startswith("target") and not d.startswith(".build") and not d.startswith(".")
            ]

            for file in files:
                if file in IGNORED_FILENAMES:
                    continue
                ext = os.path.splitext(file)[1].lower()
                if ext in SOURCE_EXTENSIONS:
                    file_path = Path(root) / file

                    # Skip symlinks (e.g. symlinked vendor C headers)
                    if file_path.is_symlink():
                        continue

                    try:
                        with open(file_path, "r", encoding="utf-8", errors="ignore") as f:
                            loc = sum(1 for _ in f)
                        
                        scanned_files += 1
                        total_loc += loc
                        rel_path = file_path.relative_to(root_dir)

                        if loc > max_loc:
                            violations.append((rel_path, loc))
                    except Exception as e:
                        print(f"{C_YELLOW}⚠️  Warning: Unable to read {file_path}: {e}{C_RESET}", file=sys.stderr)

    return scanned_files, total_loc, violations

def main():
    repo_root = get_repo_root()
    target_dir = repo_root
    min_files = 1
    max_loc = MAX_LOC_THRESHOLD
    
    if "--dir" in sys.argv:
        idx = sys.argv.index("--dir")
        if idx + 1 < len(sys.argv):
            target_dir = Path(sys.argv[idx + 1]).resolve()
            
    if "--min-files" in sys.argv:
        idx = sys.argv.index("--min-files")
        if idx + 1 < len(sys.argv):
            min_files = int(sys.argv[idx + 1])
            
    if "--max-loc" in sys.argv:
        idx = sys.argv.index("--max-loc")
        if idx + 1 < len(sys.argv):
            max_loc = int(sys.argv[idx + 1])

    print(f"{C_CYAN}{C_BOLD}======================================================================{C_RESET}")
    print(f"{C_CYAN}{C_BOLD}   TTZip Single-File LOC Defense Gate (Hard Threshold: {max_loc} LOC)  {C_RESET}")
    print(f"{C_CYAN}{C_BOLD}======================================================================{C_RESET}")
    print(f"Scanning target directories: {', '.join(SOURCE_DIRS)} (under {target_dir})")

    scanned_files, total_loc, violations = scan_loc_gate(target_dir, max_loc=max_loc)

    print(f"Scanned {scanned_files} source files ({total_loc:,} total lines of code).")

    if scanned_files < min_files:
        print(f"\n{C_RED}{C_BOLD}❌ LOC GATE FAILED: Scanned {scanned_files} files, below required baseline of {min_files} files!{C_RESET}\n")
        print(f"{C_RED}Possible path error or missing source files under: {target_dir}{C_RESET}")
        sys.exit(2)

    if violations:
        print(f"\n{C_RED}{C_BOLD}❌ LOC GATE FAILED: Found {len(violations)} monolithic file(s) exceeding {max_loc} LOC!{C_RESET}\n")
        print(f"{'LOC':>8} | File Path")
        print("-" * 70)
        for rel_path, loc in sorted(violations, key=lambda x: -x[1]):
            print(f"{C_RED}{loc:>8} LOC{C_RESET} | {rel_path}")
        print("-" * 70)
        print(f"\n{C_YELLOW}💡 Action required: Refactor and decompose violating files into smaller SRP-compliant extensions or modules.{C_RESET}")
        sys.exit(1)

    print(f"{C_GREEN}{C_BOLD}✅ [PASS] All {scanned_files} source files are clean and under the {max_loc} LOC threshold.{C_RESET}\n")
    sys.exit(0)

if __name__ == "__main__":
    main()
