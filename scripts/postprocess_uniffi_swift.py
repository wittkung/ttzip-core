#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine for macOS.

import sys

def postprocess(swift_path):
    with open(swift_path, "r", encoding="utf-8") as f:
        content = f.read()

    # 1. Add CTTZipBridge fallback import if not present
    target_import = """#if canImport(ttzip_engineFFI)
    import ttzip_engineFFI
#endif"""
    replacement_import = """#if canImport(ttzip_engineFFI)
    import ttzip_engineFFI
#elseif canImport(CTTZipBridge)
    import CTTZipBridge
#endif"""
    if "import CTTZipBridge" not in content:
        content = content.replace(target_import, replacement_import)

    # 2. Fix mutable globals for Swift 6 strict concurrency (idempotent)
    content = content.replace("nonisolated(unsafe) ", "")
    content = content.replace("static var vtable:", "nonisolated(unsafe) static var vtable:")
    content = content.replace("fileprivate static var handleMap =", "nonisolated(unsafe) fileprivate static var handleMap =")
    content = content.replace("private var initializationResult:", "private let initializationResult:")

    # 3. Strip bare print calls from uniffi template for invariant linter compliance
    content = content.replace(
        'print("Uniffi callback interface ProgressHandler: handle missing in uniffiFree")',
        '// Uniffi callback interface ProgressHandler: handle missing in uniffiFree'
    )

    with open(swift_path, "w", encoding="utf-8") as f:
        f.write(content)
    print(f"Postprocessed {swift_path} for Swift 6 concurrency.")

if __name__ == "__main__":
    if len(sys.argv) > 1:
        postprocess(sys.argv[1])
