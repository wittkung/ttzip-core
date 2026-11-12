// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import TTZipCore

// MARK: - Enterprise 100-Scenario High-Pressure Matrix

func execute100ScenarioMatrix(jsonOut: String?) -> Bool {
    print("⚡️ Executing TTZip Enterprise 100-Scenario High-Pressure Benchmark Matrix...")
    print("   Evaluating 7 Industrial Categories | Mach Kernel Task RSS Invariant: Delta <= 64MB")
    print("----------------------------------------------------------------------------------------------------------------------------------")
    print(String(format: "%-8@ | %-12@ | %-6@ | %-32@ | %-12@ | %-12@ | %-8@ | %-6@", "ID", "Category", "Fmt", "Scenario Name", "Create MB/s", "Extract MB/s", "Savings", "Status"))
    print("----------------------------------------------------------------------------------------------------------------------------------")

    do {
        let report = try ttzipBenchRunAllScenarios()
        var reportRows: [[String: Any]] = []

        for pt in report.points {
            let statusStr = pt.passedInvariants ? "✅ PASS" : "❌ FAIL"
            let savingsStr = String(format: "%5.1f%%", pt.spaceSavingsPct)
            print(String(
                format: "%-8@ | %-12@ | %-6@ | %-32@ | %9.1f MB/s | %9.1f MB/s | %-8@ | %@",
                pt.id,
                pt.category,
                pt.format,
                String(pt.displayName.prefix(32)),
                pt.createThroughputMbs,
                pt.extractThroughputMbs,
                savingsStr,
                statusStr
            ))

            reportRows.append([
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
                "create_duration_micros": pt.createDurationMicros,
                "extract_duration_micros": pt.extractDurationMicros,
                "is_encrypted": pt.isEncrypted,
                "is_split": pt.isSplit,
                "is_solid": pt.isSolid,
                "passed_invariants": pt.passedInvariants
            ])
        }

        print("----------------------------------------------------------------------------------------------------------------------------------")
        print(String(
            format: "📊 Matrix Summary: %d Scenarios Evaluated | Peak Create: %7.1f MB/s | Peak Extract: %7.1f MB/s | Gate: %@",
            report.totalScenariosEvaluated,
            report.peakCreateThroughputMbs,
            report.peakExtractThroughputMbs,
            report.allInvariantsPassed ? "✅ ALL PASSED" : "❌ GATE FAILED"
        ))
        print("----------------------------------------------------------------------------------------------------------------------------------")

        if let path = jsonOut {
            let telemetryPayload: [String: Any] = [
                "total_scenarios_evaluated": report.totalScenariosEvaluated,
                "timestamp_epoch_secs": report.timestampEpochSecs,
                "peak_create_throughput_mbs": report.peakCreateThroughputMbs,
                "peak_extract_throughput_mbs": report.peakExtractThroughputMbs,
                "all_invariants_passed": report.allInvariantsPassed,
                "scenarios": reportRows
            ]
            if let data = try? JSONSerialization.data(withJSONObject: telemetryPayload, options: .prettyPrinted) {
                try? data.write(to: URL(fileURLWithPath: path))
                print("\n📄 Enterprise 100-Scenario JSON Report written to: \(path)")
            }
        }

        return report.allInvariantsPassed && report.totalScenariosEvaluated == 100
    } catch {
        print("❌ Fatal error executing 100-scenario matrix: \(error)")
        return false
    }
}

// MARK: - Matrix Gate (8 Mathematical Synthetic Datasets)

func executeMatrixGate(corpusSizeMB: Int, jsonOut: String?) -> Bool {
    print("⚡️ Executing TTZip Matrix Gate across 8 Mathematical Synthetic Corpus Types...")
    print("   Payload Size per Corpus: \(corpusSizeMB) MB | Pinned Memory (withUnsafeBytes)")
    print("------------------------------------------------------------------------------------------------------------------")
    print(String(format: "%-32@ | %-12@ | %-12@ | %-12@ | %-12@ | %-8@", "Corpus Type", "CRC32 GB/s", "Adler GB/s", "Defl Comp", "Defl Decomp", "Status"))
    print("------------------------------------------------------------------------------------------------------------------")

    let targetBytes = max(1024 * 1024, corpusSizeMB * 1024 * 1024)
    let sizeMB = Double(targetBytes) / (1024.0 * 1024.0)
    var allPassed = true
    var reportRows: [[String: Any]] = []

    for corpusType in SyntheticCorpusType.allCases {
        let (rowPassed, rowDict) = evaluateMatrixCorpusRow(
            corpusType: corpusType,
            targetBytes: targetBytes,
            sizeMB: sizeMB
        )
        if !rowPassed {
            allPassed = false
        }
        if let rowDict = rowDict {
            reportRows.append(rowDict)
        }
    }

    print("------------------------------------------------------------------------------------------------------------------")
    if allPassed {
        print("✅ MATRIX GATE PASSED: All 8 synthetic datasets verified against hardware & codec invariants.")
    } else {
        print("❌ MATRIX GATE FAILED: One or more synthetic corpora failed throughput or integrity gates.")
    }

    if let path = jsonOut,
       let data = try? JSONSerialization.data(withJSONObject: reportRows, options: .prettyPrinted) {
        try? data.write(to: URL(fileURLWithPath: path))
        print("\n📄 Matrix Telemetry JSON exported to: \(path)")
    }

    return allPassed
}

private func evaluateMatrixCorpusRow(
    corpusType: SyntheticCorpusType,
    targetBytes: Int,
    sizeMB: Double
) -> (passed: Bool, row: [String: Any]?) {
    let rawData = SyntheticCorpusGenerator.generate(type: corpusType, size: targetBytes)
    let (crcThroughputGBs, adlerThroughputGBs, checksumOk) = measurePinnedChecksums(rawData: rawData, sizeMB: sizeMB)

    let tComp0 = DispatchTime.now().uptimeNanoseconds
    guard let compressed = AppleLibcompressionAccelerator.shared.compressData(rawData, level: 6) else {
        print(String(format: "%-32@ | %-12@ | %-12@ | %-12@ | %-12@ | ❌ FAIL", corpusType.displayName, "ERR", "ERR", "ERR", "ERR"))
        return (false, nil)
    }
    let compSec = Double(max(1, DispatchTime.now().uptimeNanoseconds - tComp0)) / 1_000_000_000.0
    let deflCompMBs = sizeMB / max(0.000001, compSec)

    let tDecomp0 = DispatchTime.now().uptimeNanoseconds
    guard let decompressed = AppleLibcompressionAccelerator.shared.decompressData(compressed, originalSize: rawData.count) else {
        print(String(format: "%-32@ | %-12.2f | %-12.2f | %-12.1f | %-12@ | ❌ FAIL", corpusType.displayName, crcThroughputGBs, adlerThroughputGBs, deflCompMBs, "ERR"))
        return (false, nil)
    }
    let decompSec = Double(max(1, DispatchTime.now().uptimeNanoseconds - tDecomp0)) / 1_000_000_000.0
    let deflDecompMBs = sizeMB / max(0.000001, decompSec)

    let rowPassed = checksumOk
        && decompressed == rawData
        && crcThroughputGBs >= 0.5
        && adlerThroughputGBs >= 0.5
        && deflCompMBs >= minimumExpectedCompressionMBs(corpusType)

    let statusStr = rowPassed ? "✅ PASS" : "⚠️ WARN"
    print(String(
        format: "%-32@ | %-10.2f GB/s | %-10.2f GB/s | %-9.1f MB/s | %-9.1f MB/s | %@",
        corpusType.rawValue, crcThroughputGBs, adlerThroughputGBs, deflCompMBs, deflDecompMBs, statusStr
    ))

    let row: [String: Any] = [
        "corpus_id": corpusType.rawValue,
        "corpus_name": corpusType.displayName,
        "size_mb": sizeMB,
        "crc32_gbs": crcThroughputGBs,
        "adler32_gbs": adlerThroughputGBs,
        "deflate_compress_mbs": deflCompMBs,
        "deflate_decompress_mbs": deflDecompMBs,
        "ratio": Double(compressed.count) / Double(rawData.count),
        "passed": rowPassed
    ]
    return (rowPassed, row)
}

private func measurePinnedChecksums(
    rawData: Data,
    sizeMB: Double
) -> (crcGBs: Double, adlerGBs: Double, valid: Bool) {
    var crcThroughputGBs: Double = 0.0
    var adlerThroughputGBs: Double = 0.0
    var valid = true

    rawData.withUnsafeBytes { rawBuffer in
        guard let basePtr = rawBuffer.baseAddress else { return }
        var bestCrcSec = Double.greatestFiniteMagnitude
        for _ in 0..<2 {
            let t0 = DispatchTime.now().uptimeNanoseconds
            let crc = HardwareChecksumAdapter.crc32(for: rawData)
            let t1 = DispatchTime.now().uptimeNanoseconds
            if crc == 0 && !rawData.isEmpty { valid = false }
            bestCrcSec = min(bestCrcSec, Double(max(1, t1 - t0)) / 1_000_000_000.0)
        }
        crcThroughputGBs = (sizeMB / 1024.0) / max(0.000001, bestCrcSec)

        var bestAdlerSec = Double.greatestFiniteMagnitude
        for _ in 0..<2 {
            let t0 = DispatchTime.now().uptimeNanoseconds
            let adler = HardwareChecksumAdapter.adler32(for: rawData)
            let t1 = DispatchTime.now().uptimeNanoseconds
            if adler == 0 && !rawData.isEmpty { valid = false }
            bestAdlerSec = min(bestAdlerSec, Double(max(1, t1 - t0)) / 1_000_000_000.0)
        }
        adlerThroughputGBs = (sizeMB / 1024.0) / max(0.000001, bestAdlerSec)
        _ = basePtr
    }
    return (crcThroughputGBs, adlerThroughputGBs, valid)
}

private func minimumExpectedCompressionMBs(_ corpusType: SyntheticCorpusType) -> Double {
    switch corpusType {
    case .dna, .literals:
        return 3.0
    case .realisticRgb:
        return 8.0
    case .machOBinary, .whiteNoise:
        return 15.0
    default:
        return 30.0
    }
}

// MARK: - Scenario Gate (Real-World Pipeline Stress)

func executeScenarioGate(jsonOut: String?) -> Bool {
    return execute100ScenarioMatrix(jsonOut: jsonOut)
}
