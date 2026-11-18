// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// RAII ，
public final class IsolatedTempSandbox: @unchecked Sendable {
    public let url: URL
    private var isCleaned: Bool = false
    
    public var path: String {
        return url.path
    }
    
    public init(prefix: String = "sandbox") throws {
        let uniqueDirName = "TTZip_\(prefix)_\(UUID().uuidString)"
        self.url = FileManager.default.temporaryDirectory.appendingPathComponent(uniqueDirName)
        try FileManager.default.createDirectory(at: self.url, withIntermediateDirectories: true)
    }
    
    /// Validates expected behavior and invariants.
    public func createSubdirectory(_ name: String) throws -> URL {
        let subDir = url.appendingPathComponent(name)
        try FileManager.default.createDirectory(at: subDir, withIntermediateDirectories: true)
        return subDir
    }
    
    /// Validates expected behavior and invariants.
    public func fileURL(named filename: String) -> URL {
        return url.appendingPathComponent(filename)
    }
    
    /// Validates expected behavior and invariants.
    public func cleanup() {
        guard !isCleaned else { return }
        isCleaned = true
        try? FileManager.default.removeItem(at: url)
    }
    
    deinit {
        cleanup()
    }
}
