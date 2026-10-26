// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// Synchronous symbol access for the legacy raw-handle API, not a snapshot owner.
internal struct VfsSnapshotBridge: Sendable {
    typealias OpenFunction = (
        UnsafePointer<CChar>, UnsafePointer<CChar>?, UnsafeMutableRawPointer
    ) -> OpaquePointer?
    typealias Lookup = (String) -> OpenFunction?
    typealias WithSymbols = @Sendable (_ body: (Lookup) throws -> OpaquePointer) throws -> OpaquePointer

    let withSymbols: WithSymbols

    static let live = VfsSnapshotBridge(withSymbols: { body in
        guard let process = dlopen(nil, RTLD_LAZY) else {
            throw VfsSnapshotBridge.unavailable()
        }
        // Symbols are borrowed only during body; the process handle covers their invocation.
        defer { dlclose(process) }
        return try body { name in
            guard let symbol = dlsym(process, name) else { return nil }
            typealias NativeOpen = @convention(c) (
                UnsafePointer<CChar>, UnsafePointer<CChar>?, UnsafeMutableRawPointer
            ) -> OpaquePointer?
            let function = unsafeBitCast(symbol, to: NativeOpen.self)
            return { path, password, view in function(path, password, view) }
        }
    })

    func open(archivePath: String, password: String?) throws -> OpaquePointer {
        try withSymbols { lookup in
            guard let open = lookup("ttzip_vfs_open_compact_catalog")
                ?? lookup("ttzip_vfs_open_snapshot") else {
                throw Self.unavailable()
            }
            // Existing ABI view is 48B/8-aligned. Core has no imported shared C view yet.
            let view = UnsafeMutableRawPointer.allocate(byteCount: 48, alignment: 8)
            defer { view.deallocate() }
            let lease = archivePath.withCString { path in
                if let password {
                    return password.withCString { open(path, $0, view) }
                }
                return open(path, nil, view)
            }
            // The legacy C API provides no detailed failure status. Do not guess a cause.
            guard let lease else { throw ArchiveError.openFailed }
            return lease
        }
    }

    private static func unavailable() -> ArchiveError {
        .engineFailure(
            code: RustTTZipStatusCode.errUnsupportedFeature.rawValue,
            message: "VFS snapshot bridge is unavailable"
        )
    }
}
