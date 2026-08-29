// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class Scenario100BenchmarkMatrixTests: XCTestCase {

    func test100ScenarioMatrixExecutionAndInvariants() throws {
        let report = try ttzipBenchRunAllScenarios()

        XCTAssertEqual(report.totalScenariosEvaluated, 100, "Scenario matrix must contain exactly 100 benchmark points")
        XCTAssertTrue(report.allInvariantsPassed, "All 100 scenarios must pass integrity and memory boundary invariants")
        XCTAssertGreaterThan(report.peakCreateThroughputMbs, 0.0, "Peak create throughput must be positive")
        XCTAssertGreaterThan(report.peakExtractThroughputMbs, 0.0, "Peak extract throughput must be positive")

        // Verify distinct categories
        let categories = Set(report.points.map { $0.category })
        XCTAssertTrue(categories.contains("Security"), "Matrix must contain Security category")
        XCTAssertTrue(categories.contains("SolidBlock"), "Matrix must contain SolidBlock category")
        XCTAssertTrue(categories.contains("SplitVolume"), "Matrix must contain SplitVolume category")
        XCTAssertTrue(categories.contains("Topology"), "Matrix must contain Topology category")
        XCTAssertTrue(categories.contains("Lifecycle"), "Matrix must contain Lifecycle category")
        XCTAssertTrue(categories.contains("Resilience"), "Matrix must contain Resilience category")
        XCTAssertTrue(categories.contains("Container"), "Matrix must contain Container category")

        // Verify individual point fields
        for pt in report.points {
            XCTAssertFalse(pt.id.isEmpty, "Scenario ID must not be empty")
            XCTAssertFalse(pt.displayName.isEmpty, "Scenario displayName must not be empty")
            XCTAssertFalse(pt.format.isEmpty, "Scenario format must not be empty")
            XCTAssertTrue(pt.passedInvariants, "Scenario \(pt.id) (\(pt.displayName)) failed invariants")
        }
    }

    func testScenarioReportJsonSerialization() throws {
        let report = try ttzipBenchRunAllScenarios()

        let scenarioDicts: [[String: Any]] = report.points.map { pt in
            [
                "id": pt.id,
                "category": pt.category,
                "format": pt.format,
                "display_name": pt.displayName,
                "options_summary": pt.optionsSummary,
                "original_size_bytes": pt.originalSizeBytes,
                "output_size_bytes": pt.outputSizeBytes,
                "space_savings_pct": pt.spaceSavingsPct,
                "create_throughput_mbs": pt.createThroughputMbs,
                "extract_throughput_mbs": pt.extractThroughputMbs,
                "passed_invariants": pt.passedInvariants
            ]
        }

        let root: [String: Any] = [
            "total_scenarios_evaluated": report.totalScenariosEvaluated,
            "timestamp_epoch_secs": report.timestampEpochSecs,
            "peak_create_throughput_mbs": report.peakCreateThroughputMbs,
            "peak_extract_throughput_mbs": report.peakExtractThroughputMbs,
            "all_invariants_passed": report.allInvariantsPassed,
            "scenarios": scenarioDicts
        ]

        let jsonData = try JSONSerialization.data(withJSONObject: root, options: .prettyPrinted)
        XCTAssertGreaterThan(jsonData.count, 1024, "Serialized JSON report must be non-trivial")

        let deserialized = try JSONSerialization.jsonObject(with: jsonData) as? [String: Any]
        XCTAssertNotNil(deserialized)
        XCTAssertEqual(deserialized?["total_scenarios_evaluated"] as? Int, 100)
    }
}
