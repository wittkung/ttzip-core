// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import XCTest
@testable import TTZipCore

final class RustVfsSessionSearchBenchmarkTests: XCTestCase {

    /// The absolute wall-clock ceiling for a 10k entry search.
    ///
    /// The ceiling is the same on every machine and in every environment. A
    /// shared runner is slower than a workstation, so the measurement must not
    /// depend on the machine being fast: the budget below is the *local*
    /// measurement of the same work, and a runner that cannot keep up fails
    /// the test rather than silently accepting a ten times slower search.
    private static let absoluteBudget: TimeInterval = 0.05

    /// The largest growth factor tolerated when the input size doubles.
    ///
    /// `fuzzy_search` scans every arena node once and sorts the matches, so it
    /// is linear in the number of entries. Doubling the input therefore
    /// doubles the work, and the measured ratio stays close to 2. A quadratic
    /// scan quadruples the work instead, which a ratio of 4 exposes. The
    /// ceiling sits above the linear expectation and below the quadratic one
    /// so the test separates the two growth classes without being sensitive
    /// to the noise of a single timing sample.
    private static let maxGrowthRatio: Double = 3.0

    /// Builds a session over `count` synthetic entries.
    private func makeSession(count: Int) -> RustVfsSession? {
        var entries: [ArchiveEntry] = []
        entries.reserveCapacity(count)
        for i in 0..<count {
            entries.append(ArchiveEntry(
                path: "src/deep/module_\(i)/component_\(i).swift",
                uncompressedSize: 1024,
                isDirectory: false,
                detectedEncoding: "UTF-8",
                modificationDate: Date(),
                isEncrypted: false
            ))
        }
        return RustVfsSession(entries: entries, rootName: "project")
    }

    /// The fastest observed search time over `samples` runs.
    ///
    /// The minimum is used because it is the sample least disturbed by the
    /// scheduler: preemption, interrupts and contention can only make a run
    /// slower, so the fastest run is the closest estimate of the work itself.
    private func fastestSearch(session: RustVfsSession, query: String, samples: Int) -> (elapsed: TimeInterval, hits: Int) {
        var best = TimeInterval.infinity
        var hits = 0
        for _ in 0..<samples {
            let start = Date()
            let results = session.fuzzySearch(query: query)
            let elapsed = Date().timeIntervalSince(start)
            if elapsed < best {
                best = elapsed
                hits = results.count
            }
        }
        return (best, hits)
    }

    func testVfsSessionSearchPerformance10k() {
        guard let session = makeSession(count: 10_000) else {
            XCTFail("Failed to build RustVfsSession")
            return
        }

        // Warmup pass: the first call pays for the lazily materialised arena.
        _ = session.fuzzySearch(query: "warmup")

        let measurement = fastestSearch(session: session, query: "comp_99", samples: 3)
        XCTAssertGreaterThan(measurement.hits, 0)

        XCTAssertLessThan(
            measurement.elapsed,
            Self.absoluteBudget,
            "10k entry search must complete in < \(Int(Self.absoluteBudget * 1000))ms"
        )
    }

    /// Verifies the search grows linearly with the input size.
    ///
    /// A wall-clock ceiling on one input size cannot tell a linear scan from a
    /// quadratic one on a machine that is merely slow, and it cannot tell a
    /// fast machine running a quadratic scan from a slow machine running a
    /// linear one. Comparing the same query across two input sizes measures
    /// the growth instead of the speed: the ratio separates the growth classes
    /// on any machine, and it is unaffected by how fast that machine is.
    func testVfsSessionSearchGrowsLinearly() {
        let smallCount = 10_000
        let largeCount = smallCount * 2

        guard let smallSession = makeSession(count: smallCount) else {
            XCTFail("Failed to build RustVfsSession")
            return
        }
        guard let largeSession = makeSession(count: largeCount) else {
            XCTFail("Failed to build RustVfsSession")
            return
        }

        _ = smallSession.fuzzySearch(query: "warmup")
        _ = largeSession.fuzzySearch(query: "warmup")

        let small = fastestSearch(session: smallSession, query: "comp_99", samples: 5)
        let large = fastestSearch(session: largeSession, query: "comp_99", samples: 5)

        XCTAssertGreaterThan(small.hits, 0)
        XCTAssertGreaterThan(large.hits, 0)

        // A search that takes no measurable time cannot produce a meaningful
        // ratio; the timer resolution would dominate the measurement.
        guard small.elapsed > 0.000_001 else {
            XCTFail("The \(smallCount) entry search was too fast to measure")
            return
        }

        let ratio = large.elapsed / small.elapsed
        XCTAssertLessThan(
            ratio,
            Self.maxGrowthRatio,
            "Doubling the input from \(smallCount) to \(largeCount) entries grew the search "
                + "by \(String(format: "%.2f", ratio))x; a linear scan stays near 2x and a "
                + "quadratic scan reaches 4x"
        )
    }
}
