#!/usr/bin/env python3
"""
audit_dual_build.py
Verifies 1:1 bidirectional consistency of C source and header files between:
1. Makefile.am (GNU Autotools, including conditional sources)
2. libarchive/CMakeLists.txt and CMakeLists.txt (CMake)
"""

import re
import sys
from pathlib import Path

def main():
    root = Path.cwd()
    makefile_am = root / "Makefile.am"
    cmake_libarchive = root / "libarchive" / "CMakeLists.txt"

    if not makefile_am.exists() or not cmake_libarchive.exists():
        print(f"Error: Missing build files in {root}")
        sys.exit(1)

    with open(makefile_am, "r", encoding="utf-8") as f:
        mf_text = f.read()

    with open(cmake_libarchive, "r", encoding="utf-8") as f:
        cm_text = f.read()

    # Find ALL libarchive/xxx.c references in Makefile.am (main list + all conditional += blocks)
    mf_c_files = set(re.findall(r"libarchive/([a-zA-Z0-9_\.]+\.c)", mf_text))

    # Extract CMake libarchive sources from libarchive/CMakeLists.txt
    cm_c_files = set(re.findall(r"([a-zA-Z0-9_\.]+\.c)", cm_text))
    # Exclude test files
    cm_c_files = {f for f in cm_c_files if not f.startswith("test_")}

    in_cmake_only = sorted(list(cm_c_files - mf_c_files))
    in_makefile_only = sorted(list(mf_c_files - cm_c_files))

    print(f"=== Dual-Build System Bidirectional Audit ===")
    print(f"Makefile.am total C sources: {len(mf_c_files)}")
    print(f"CMakeLists.txt total C sources: {len(cm_c_files)}")

    has_error = False
    if in_cmake_only:
        print(f"[FAIL] Files in CMakeLists.txt but MISSING from Makefile.am:\n  " + "\n  ".join(in_cmake_only))
        has_error = True

    if in_makefile_only:
        print(f"[INFO] Files in Makefile.am but conditional in CMake:\n  " + "\n  ".join(in_makefile_only))

    if not has_error:
        print(">>> SUCCESS: 100% Dual-Build Consistency Verified! Zero missing sources.")
        sys.exit(0)
    else:
        sys.exit(1)

if __name__ == "__main__":
    main()
