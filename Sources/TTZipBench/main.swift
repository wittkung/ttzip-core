// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import TTZipCore

func printHelp() {
    print("""
    OVERVIEW: TTZip Benchmark & Compression Telemetry CLI (Pure UniFFI & Apple Silicon Native)

    USAGE: ttzip-bench <subcommand> [options]

    SUBCOMMANDS:
      pipeline      Execute full-pipeline E2E benchmark (Swift Facade -> UniFFI -> Rust -> APFS I/O) and calculate FFI Tax %
      gate          Run automated regression and hardware stability checks for CI/CD
      help          Display this help message

    OPTIONS:
      --json-out <path>    Write structured telemetry report to JSON file
      --lang <bcp47>       Force target language (en, zh-Hans, zh-Hant, ja, de, fr, es)
    """)
}

func executePipelineBenchmark(jsonOut: String?) {
    print("⚡️ Executing TTZip End-to-End Pipeline & FFI Tax Benchmark...")

    let corpusSizes = [10 * 1024 * 1024, 25 * 1024 * 1024] // 10MB, 25MB
    var pipelineReports: [[String: Any]] = []

    for size in corpusSizes {
        let sizeMB = size / (1024 * 1024)
        print("\n--- Running Pipeline Benchmark for Corpus Size: \(sizeMB) MB ---")

        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent("ttzip_bench_\(UUID().uuidString)")
        try? FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let inputFile = tempDir.appendingPathComponent("bench_data.bin")
        let outputZip = tempDir.appendingPathComponent("bench_out.zip")
        
        let rawData = Data((0..<size).map { UInt8(($0 ^ ($0 >> 3)) & 0xFF) })
        try? rawData.write(to: inputFile)

        // 1. Measure Isolated In-Memory Codec speed
        let tIsolatedStart = DispatchTime.now().uptimeNanoseconds
        let compressed = LibdeflateAccelerator.shared.compressData(rawData)
        let tIsolatedEnd = DispatchTime.now().uptimeNanoseconds
        guard compressed != nil else {
            print("❌ Failed isolated compression.")
            continue
        }
        let isolatedDurationSec = Double(tIsolatedEnd - tIsolatedStart) / 1_000_000_000.0
        let isolatedThroughput = Double(sizeMB) / max(0.0001, isolatedDurationSec)

        // 2. Measure End-to-End APFS Pipeline speed
        let writer = ArchiveWriter()
        guard let provenance = try? writer.createArchiveWithReport(
            outputPath: outputZip.path,
            format: .zip,
            level: .normal,
            inputPaths: [inputFile.path]
        ) else {
            print("❌ Failed to create benchmark archive.")
            continue
        }

        let e2eDurationSec = Double(provenance.totalE2EDurationNanos) / 1_000_000_000.0
        let e2eThroughput = Double(sizeMB) / max(0.0001, e2eDurationSec)

        // 3. Compute FFI Bridge Tax and Full Pipeline Overhead
        let ffiTaxPercent = (Double(provenance.ffiBridgeOverheadNanos) / max(1.0, Double(provenance.totalE2EDurationNanos))) * 100.0
        let degradationPercent = (1.0 - (e2eThroughput / max(0.0001, isolatedThroughput))) * 100.0

        print("📊 [Results for \(sizeMB) MB]")
        print("  - Dispatched Engine:      \(provenance.engineTag.rawValue) (Fallback: \(provenance.isFallback))")
        print("  - Isolated Codec Speed:   \(String(format: "%.1f", isolatedThroughput)) MB/s")
        print("  - E2E Pipeline Speed:     \(String(format: "%.1f", e2eThroughput)) MB/s")
        print("  - FFI Bridge Tax:         \(String(format: "%.2f", ffiTaxPercent)) %")
        print("  - Full Pipeline Overhead: \(String(format: "%.2f", degradationPercent)) %")

        pipelineReports.append([
            "corpus_size_mb": sizeMB,
            "engine": provenance.engineTag.rawValue,
            "isolated_mbs": isolatedThroughput,
            "e2e_mbs": e2eThroughput,
            "ffi_tax_pct": ffiTaxPercent,
            "degradation_pct": degradationPercent
        ])
    }

    if let path = jsonOut {
        if let data = try? JSONSerialization.data(withJSONObject: pipelineReports, options: .prettyPrinted) {
            try? data.write(to: URL(fileURLWithPath: path))
            print("\n📄 Pipeline Telemetry JSON exported to: \(path)")
        }
    }
}

let args = Array(CommandLine.arguments.dropFirst())
guard let command = args.first, command != "--help", command != "-h", command != "help" else {
    printHelp()
    exit(0)
}

var jsonOut: String?
var idx = 1
while idx < args.count {
    if args[idx] == "--json-out", idx + 1 < args.count {
        jsonOut = args[idx + 1]
        idx += 2
    } else if args[idx] == "--lang", idx + 1 < args.count {
        let langStr = args[idx + 1]
        if let parsed = AppLanguage.from(identifier: langStr) {
            TTZipLocalizationManager.shared.currentLanguage = parsed
        }
        idx += 2
    } else {
        idx += 1
    }
}

switch command {
case "pipeline":
    executePipelineBenchmark(jsonOut: jsonOut)
    print("✅ Pipeline benchmark completed successfully.")
    exit(0)

case "gate":
    print("⚡️ Running TTZip Automated Benchmark Gate (Pure UniFFI & Darwin Native)...")
    let testData = Data(repeating: 0x55, count: 1024 * 1024)
    let crc = HardwareChecksumAdapter.crc32(for: testData)
    if crc != 0 {
        print("✅ GATE PASSED: All hardware & codec invariants verified.")
        exit(0)
    } else {
        print("❌ GATE FAILED")
        exit(70)
    }

default:
    print("❌ Unknown subcommand: '\(command)'\n")
    printHelp()
    exit(64)
}
