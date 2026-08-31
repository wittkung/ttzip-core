// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import TTZipCore

// MARK: - Declarative A/B Microkernel Benchmark Suite

func executeAbBenchmark(
    targetFilter: String,
    corpusUri: String,
    sizeBytes: UInt64,
    baselineJsonPath: String?,
    rounds: UInt32,
    format: String,
    jsonOut: String?,
    snapshotOut: String? = nil
) -> Bool {
    let effectiveRounds = max(4, rounds)
    let config = UniFfiAbOrchestratorConfig(
        warmupRounds: min(3, max(1, effectiveRounds / 4)),
        measurementRounds: effectiveRounds,
        maxAllowedRegression: 3.0,
        pValueThreshold: 0.05,
        hampelFilter: true,
        hampelK: 3.0,
        targetRsePct: 0.5
    )

    do {
        let reportJson: String
        if let baselinePath = baselineJsonPath {
            let baselineData = try Data(contentsOf: URL(fileURLWithPath: baselinePath))
            guard let baselineStr = String(data: baselineData, encoding: .utf8) else {
                print("❌ Failed to read baseline JSON from '\(baselinePath)'")
                return false
            }
            reportJson = try ttzipBenchCompareAgainstBaselineJson(
                targetFilter: targetFilter,
                corpusUri: corpusUri,
                sizeBytes: sizeBytes,
                baselineJson: baselineStr,
                config: config
            )
        } else {
            reportJson = try ttzipBenchRunAbBenchmarkJson(
                targetFilter: targetFilter,
                corpusUri: corpusUri,
                sizeBytes: sizeBytes,
                config: config
            )
        }

        // Render formatted report using Rust UniFFI exporters
        let rendered: String
        switch format.lowercased() {
        case "json":
            rendered = try ttzipBenchRenderAbJson(reportJson: reportJson)
        case "markdown", "md":
            rendered = try ttzipBenchRenderAbMarkdown(reportJson: reportJson)
        case "table", "ascii":
            fallthrough
        default:
            rendered = try ttzipBenchRenderAbAscii(reportJson: reportJson, ansiColor: true)
        }

        print(rendered)

        if let outPath = jsonOut {
            let formattedJson = try ttzipBenchRenderAbJson(reportJson: reportJson)
            try formattedJson.write(to: URL(fileURLWithPath: outPath), atomically: true, encoding: .utf8)
            print("📄 Telemetry JSON exported to: \(outPath)")
        }

        if let snapPath = snapshotOut {
            let snapJson = try ttzipBenchCreateBaselineSnapshot(reportJson: reportJson, useCandidate: false)
            try snapJson.write(to: URL(fileURLWithPath: snapPath), atomically: true, encoding: .utf8)
            print("💾 Golden baseline snapshot exported to: \(snapPath)")
        }

        // Parse overall quality gate verdict from reportJson
        if let data = reportJson.data(using: .utf8),
           let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            let summary = dict["summary"] as? [String: Any]
            let overallPassed = (dict["overall_passed"] as? Bool) ?? (summary?["overall_passed"] as? Bool) ?? true
            return overallPassed
        }

        return true
    } catch {
        print("❌ A/B Benchmark execution error: \(error)")
        return false
    }
}
