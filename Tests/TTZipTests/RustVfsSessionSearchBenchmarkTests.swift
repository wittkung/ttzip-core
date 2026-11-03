// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import XCTest
@testable import TTZipCore

final class RustVfsSessionSearchBenchmarkTests: XCTestCase {
    
    func testVfsSessionSearchPerformance10k() {
        var entries: [ArchiveEntry] = []
        entries.reserveCapacity(10_000)
        
        for i in 0..<10_000 {
            entries.append(ArchiveEntry(
                path: "src/deep/module_\(i)/component_\(i).swift",
                uncompressedSize: 1024,
                isDirectory: false,
                detectedEncoding: "UTF-8",
                modificationDate: Date(),
                isEncrypted: false
            ))
        }
        
        guard let session = RustVfsSession(entries: entries, rootName: "project") else {
            XCTFail("Failed to build RustVfsSession")
            return
        }
        
        // Warmup pass
        _ = session.fuzzySearch(query: "warmup")
        
        var minElapsed: TimeInterval = .infinity
        var results: [ArchiveEntry] = []
        for _ in 0..<3 {
            let start = Date()
            let r = session.fuzzySearch(query: "comp_99")
            let elapsed = Date().timeIntervalSince(start)
            if elapsed < minElapsed {
                minElapsed = elapsed
                results = r
            }
        }
        
        XCTAssertGreaterThan(results.count, 0)

        // The budget guards the algorithmic complexity, not the machine. A shared CI runner is
        // roughly an order of magnitude slower than a workstation, so the wall-clock ceiling is
        // relaxed there; an accidental quadratic scan still exceeds it by orders of magnitude.
        let isCI = ProcessInfo.processInfo.environment["CI"] != nil
        let budget: TimeInterval = isCI ? 0.5 : 0.05
        XCTAssertLessThan(minElapsed, budget, "10k entry search must complete in < \(Int(budget * 1000))ms")
    }
}
