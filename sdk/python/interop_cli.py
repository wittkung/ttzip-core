#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# Python PyO3 Headless Interop CLI Runner.

import sys
import os
from pathlib import Path

# Add python directory to sys.path
SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ttzip


def parse_format(fmt_str: str) -> str:
    s = fmt_str.lower()
    if s in ("zip", "pkzip"):
        return "zip"
    if s in ("7z", "7zip", "sevenzip"):
        return "7z"
    if s in ("tar",):
        return "tar"
    if s in ("tar.gz", "targz", "tgz", "gz"):
        return "tar.gz"
    if s in ("tar.bz2", "tarbz2", "tbz2", "bz2"):
        return "tar.bz2"
    if s in ("tar.xz", "tarxz", "txz", "xz"):
        return "tar.xz"
    if s in ("tar.zst", "tarzst", "tar.zstd", "zst"):
        return "tar.zst"
    return "zip"


def print_usage(prog: str) -> None:
    sys.stderr.write(f"Usage:\n")
    sys.stderr.write(f"  {prog} --create <format> <src> <dst> [--password <pwd>]\n")
    sys.stderr.write(f"  {prog} --extract <src> <dst> [--password <pwd>]\n")
    sys.stderr.write(f"  {prog} --version\n")


def main() -> int:
    args = sys.argv[1:]
    if not args:
        print_usage(sys.argv[0])
        return 2

    if args[0] == "--version":
        print(ttzip.version())
        return 0

    mode = None
    format_str = None
    src = None
    dst = None
    password = None

    i = 0
    while i < len(args):
        arg = args[i]
        if arg == "--create":
            mode = "create"
            if i + 3 >= len(args):
                sys.stderr.write("Error: --create requires <format> <src> <dst>\n")
                return 2
            format_str = args[i + 1]
            src = args[i + 2]
            dst = args[i + 3]
            i += 4
        elif arg == "--extract":
            mode = "extract"
            if i + 2 >= len(args):
                sys.stderr.write("Error: --extract requires <src> <dst>\n")
                return 2
            src = args[i + 1]
            dst = args[i + 2]
            i += 3
        elif arg == "--password":
            if i + 1 >= len(args):
                sys.stderr.write("Error: --password requires an argument\n")
                return 2
            password = args[i + 1]
            i += 2
        else:
            sys.stderr.write(f"Unknown argument: {arg}\n")
            print_usage(sys.argv[0])
            return 2

    if not mode:
        print_usage(sys.argv[0])
        return 2

    try:
        if mode == "create":
            fmt = parse_format(format_str)
            ttzip.compress(
                sources=src,
                destination=dst,
                format=fmt,
                level=6,
                password=password,
            )
            return 0
        elif mode == "extract":
            ttzip.extract(
                archive=src,
                destination=dst,
                password=password,
            )
            return 0
    except Exception as ex:
        sys.stderr.write(f"Error: {ex}\n")
        return 1

    return 2


if __name__ == "__main__":
    sys.exit(main())
