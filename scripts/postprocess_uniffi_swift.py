#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

import sys

def postprocess(swift_path):
    with open(swift_path, "r", encoding="utf-8") as f:
        content = f.read()

    # 0. Ensure SPDX header is present
    spdx_header = """// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

"""
    if "SPDX-License-Identifier" not in content:
        content = spdx_header + content

    # 1. Add CTTZipBridge import if not present
    if "import CTTZipBridge" not in content:
        content = content.replace(
            "#if canImport(ttzip_engineFFI)\nimport ttzip_engineFFI\n#endif",
            "import CTTZipBridge"
        )
        content = content.replace(
            "#if canImport(ttzip_engineFFI)\n    import ttzip_engineFFI\n#endif",
            "import CTTZipBridge"
        )
    content = content.replace(
        "#if canImport(ttzip_engineFFI)\n    import ttzip_engineFFI\n#elseif canImport(CTTZipBridge)\n    import CTTZipBridge\n#endif",
        "import CTTZipBridge"
    )
    content = content.replace(
        "#if canImport(ttzip_engineFFI)\nimport ttzip_engineFFI\n#elseif canImport(CTTZipBridge)\nimport CTTZipBridge\n#endif",
        "import CTTZipBridge"
    )

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

    # 4. Add Sendable conformances for UniFFI classes
    sendable_extensions = """
extension UniFfiVfsTree: @unchecked Sendable {}
extension CancellationToken: @unchecked Sendable {}
extension UniFfiPluginSigner: @unchecked Sendable {}
extension UniFfiPluginVerifier: @unchecked Sendable {}
extension UniFfiMmapReader: @unchecked Sendable {}
"""
    if "extension UniFfiVfsTree: @unchecked Sendable" not in content:
        content += sendable_extensions

    # 5. Ensure RustBuffer deallocate is inside defer to prevent leak on throw
    old_lift = """    public static func lift(_ buf: RustBuffer) throws -> SwiftType {
        var reader = createReader(data: Data(rustBuffer: buf))
        let value = try read(from: &reader)
        if hasRemaining(reader) {
            throw UniffiInternalError.incompleteData
        }
        buf.deallocate()
        return value
    }"""
    new_lift = """    public static func lift(_ buf: RustBuffer) throws -> SwiftType {
        defer { buf.deallocate() }
        var reader = createReader(data: Data(rustBuffer: buf))
        let value = try read(from: &reader)
        if hasRemaining(reader) {
            throw UniffiInternalError.incompleteData
        }
        return value
    }"""
    content = content.replace(old_lift, new_lift)

    # 6. Ensure deinit calls do not use try! to prevent fatal crashes
    content = content.replace("        try! rustCall { uniffi_", "        _ = try? rustCall { uniffi_")

    with open(swift_path, "w", encoding="utf-8") as f:
        f.write(content)
    print(f"Postprocessed {swift_path} for Swift 6 concurrency and memory safety.")

if __name__ == "__main__":
    if len(sys.argv) > 1:
        postprocess(sys.argv[1])
