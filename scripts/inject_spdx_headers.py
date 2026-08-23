#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine for macOS.

import os
import sys

SWIFT_C_HEADER = """// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

"""

SHELL_HEADER = """# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine for macOS.

"""

IGNORE_SUBSTRINGS = [
    "Vendor/",
    "Fixtures/",
    "Sources/CTTZipBridge/zopfli/",
    "Sources/CTTZipBridge/snappy/",
    "Sources/CTTZipBridge/fast-lzma2/",
    "Sources/CTTZipBridge/lzfse/",
]

def should_process(file_path):
    for ign in IGNORE_SUBSTRINGS:
        if ign in file_path:
            return False
    ext = os.path.splitext(file_path)[1].lower()
    return ext in [".swift", ".c", ".h", ".sh"]

def inject_header(file_path):
    with open(file_path, "r", encoding="utf-8", errors="ignore") as fp:
        content = fp.read()
    
    if "SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0" in content:
        return False
    
    ext = os.path.splitext(file_path)[1].lower()
    if ext in [".swift", ".c", ".h"]:
        new_content = SWIFT_C_HEADER + content.lstrip("\n")
    elif ext == ".sh":
        lines = content.splitlines(True)
        if lines and lines[0].startswith("#!"):
            shebang = lines[0]
            rest = "".join(lines[1:]).lstrip("\n")
            new_content = shebang + SHELL_HEADER + rest
        else:
            new_content = "#!/usr/bin/env bash\n" + SHELL_HEADER + content.lstrip("\n")
    else:
        return False
    
    with open(file_path, "w", encoding="utf-8") as fp:
        fp.write(new_content)
    return True

def main():
    root_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    scan_dirs = [
        os.path.join(root_dir, "Sources"),
        os.path.join(root_dir, "Tests"),
        os.path.join(root_dir, "scripts"),
    ]
    
    injected_count = 0
    scanned_count = 0
    
    for sdir in scan_dirs:
        if not os.path.exists(sdir):
            continue
        for root, _, files in os.walk(sdir):
            for f in sorted(files):
                full_path = os.path.join(root, f)
                rel_path = os.path.relpath(full_path, root_dir)
                if should_process(rel_path):
                    scanned_count += 1
                    if inject_header(full_path):
                        injected_count += 1
                        print(f"  [INJECT] {rel_path}")
                        
    print(f"Scanned {scanned_count} files. Injected SPDX headers into {injected_count} files.")

if __name__ == "__main__":
    main()
