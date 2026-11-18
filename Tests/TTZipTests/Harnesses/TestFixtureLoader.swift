// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// ( SPM Bundle.module )
public enum TestFixtureLoader {

    /// Fixtures/Encrypted URL
    public static func encryptedFixtureURL(named filename: String) throws -> URL {
        let name: String
        let ext: String

        if let dotIndex = filename.lastIndex(of: ".") {
            name = String(filename[..<dotIndex])
            ext = String(filename[filename.index(after: dotIndex)...])
        } else {
            name = filename
            ext = ""
        }

        // 1. Bundle.module
        #if SWIFT_PACKAGE
        if let resourceURL = Bundle.module.url(forResource: name, withExtension: ext, subdirectory: "Fixtures/Encrypted") {
            return resourceURL
        }
        if let resourceURL = Bundle.module.url(forResource: filename, withExtension: nil, subdirectory: "Fixtures/Encrypted") {
            return resourceURL
        }
        #endif

        if let fallbackPath = resolveFixturesURL("Encrypted/\(filename)") {
            return fallbackPath
        }

        throw NSError(
            domain: "TestFixtureLoader",
            code: 404,
            userInfo: [NSLocalizedDescriptionKey: "Test fixture '\(filename)' not found in Bundle.module or Fixtures/Encrypted/"]
        )
    }

    /// Returns the filesystem path string for an encrypted fixture.
    public static func encryptedFixturePath(named filename: String) throws -> String {
        return try encryptedFixtureURL(named: filename).path
    }

    static func resolveFixturesURL(_ subpath: String, filePath: String = #filePath) -> URL? {
        if let runfilesURL = resolveRunfilesFixtureURL(subpath) {
            return runfilesURL
        }
        return walkAncestorFixtureURL(subpath, filePath: filePath)
    }

    private static func resolveRunfilesFixtureURL(_ subpath: String) -> URL? {
        let fm = FileManager.default
        let env = ProcessInfo.processInfo.environment
        let roots = [env["TEST_SRCDIR"], env["RUNFILES_DIR"]].compactMap { $0 }.filter { !$0.isEmpty }
        let prefixes = [
            "_main/products/ttzip/core/Tests/TTZipTests/Fixtures",
            "products/ttzip/core/Tests/TTZipTests/Fixtures",
        ]
        for root in roots {
            let base = URL(fileURLWithPath: root)
            for prefix in prefixes {
                let candidate = base.appendingPathComponent(prefix).appendingPathComponent(subpath).resolvingSymlinksInPath()
                if fm.fileExists(atPath: candidate.path) {
                    return candidate
                }
            }
        }
        return nil
    }

    private static func walkAncestorFixtureURL(_ subpath: String, filePath: String) -> URL? {
        let fm = FileManager.default
        var dir = URL(fileURLWithPath: filePath).deletingLastPathComponent()
        while true {
            for rel in ["Fixtures", "Tests/TTZipTests/Fixtures"] {
                let candidate = dir.appendingPathComponent(rel).appendingPathComponent(subpath).resolvingSymlinksInPath()
                if fm.fileExists(atPath: candidate.path) {
                    return candidate
                }
            }
            let parent = dir.deletingLastPathComponent()
            if fm.fileExists(atPath: dir.appendingPathComponent("Package.swift").path)
                || parent.standardizedFileURL.path == dir.standardizedFileURL.path {
                return nil
            }
            dir = parent
        }
    }
}
