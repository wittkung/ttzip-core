#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

"""
inject_spdx_headers.py - Forwarding CLI wrapper around clean_license_headers.py
Ensures safe, deduplicated header injection and formatting.
"""

import sys
import subprocess
import os

def main():
    script_dir = os.path.dirname(os.path.abspath(__file__))
    clean_script = os.path.join(script_dir, "clean_license_headers.py")
    cmd = [sys.executable, clean_script] + sys.argv[1:]
    res = subprocess.run(cmd)
    sys.exit(res.returncode)

if __name__ == "__main__":
    main()
