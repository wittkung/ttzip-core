// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import Foundation
import TTZipCore
import CTTZipBridge

func printHelp() {
    print("""
    OVERVIEW: TTZip Benchmark & Compression Telemetry CLI (Rust Native C-ABI)

    USAGE: ttzip-bench <subcommand> [options]

    SUBCOMMANDS:
      matrix        Execute multi-engine in-memory benchmark matrix (libdeflate, zstd, lz4, lzfse, snappy, brotli, bzip2)
      scenario      Execute 24-point enterprise full-scenario matrix (encryption, split volumes, solid blocks, topologies)
      pipeline      Execute full-pipeline E2E benchmark (Swift Facade -> C-ABI -> Rust -> APFS I/O) and calculate FFI Tax %
      gate          Run automated regression and hardware stability checks for CI/CD
      plot          Generate interactive Pareto frontier charts (SVG / HTML dashboard)
      help          Display this help message

    OPTIONS (matrix, pipeline & plot):
      --json-out <path>    Write structured telemetry report to JSON file
      --svg-out <path>     Write interactive vector SVG Pareto chart
      --html-out <path>    Write self-contained Zen UI HTML dashboard
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
        let isolatedBuf = UnsafeMutablePointer<UInt8>.allocate(capacity: size * 2)
        defer { isolatedBuf.deallocate() }
        
        rawData.withUnsafeBytes { rawPtr in
            var outLen = 0
            _ = ttzip_rust_deflate_compress(
                rawPtr.baseAddress?.assumingMemoryBound(to: UInt8.self),
                size,
                isolatedBuf,
                size * 2,
                6,
                &outLen
            )
        }
        let tIsolatedEnd = DispatchTime.now().uptimeNanoseconds
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
var svgOut: String?
var htmlOut: String?
var idx = 1
while idx < args.count {
    if args[idx] == "--json-out", idx + 1 < args.count {
        jsonOut = args[idx + 1]
        idx += 2
    } else if args[idx] == "--svg-out", idx + 1 < args.count {
        svgOut = args[idx + 1]
        idx += 2
    } else if args[idx] == "--html-out", idx + 1 < args.count {
        htmlOut = args[idx + 1]
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

case "matrix", "plot":
    print("⚡️ Executing TTZip Multi-Engine Matrix Benchmark (Rust C-ABI)...")
    let bufSize = 512 * 1024
    let jsonBuffer = UnsafeMutablePointer<CChar>.allocate(capacity: bufSize)
    defer { jsonBuffer.deallocate() }

    let written = ttzip_rust_bench_run_matrix(0, jsonBuffer, bufSize)
    guard written > 0 else {
        print("❌ Benchmark matrix execution failed with status: \(written)")
        exit(1)
    }

    let jsonString = String(cString: jsonBuffer)
    if let jsonPath = jsonOut {
        try? jsonString.write(toFile: jsonPath, atomically: true, encoding: .utf8)
        print("📄 Telemetry JSON report exported to: \(jsonPath)")
    }

    if let svgPath = svgOut, let svgPtr = ttzip_rust_bench_generate_svg_pareto(0, 960, 480) {
        let svgStr = String(cString: svgPtr)
        ttzip_rust_bench_free_string(svgPtr)
        try? svgStr.write(toFile: svgPath, atomically: true, encoding: .utf8)
        print("📈 Interactive SVG Pareto chart exported to: \(svgPath)")
    }

    if let htmlPath = htmlOut, let htmlPtr = ttzip_rust_bench_generate_html_dashboard(0) {
        let htmlStr = String(cString: htmlPtr)
        ttzip_rust_bench_free_string(htmlPtr)
        try? htmlStr.write(toFile: htmlPath, atomically: true, encoding: .utf8)
        print("🌐 Self-contained Zen UI HTML Dashboard exported to: \(htmlPath)")
    }
    print("✅ Matrix benchmark completed successfully.")
    exit(0)

case "scenario":
    print("⚡️ Executing TTZip 24-Point Enterprise Full-Scenario Benchmark Matrix (Rust C-ABI)...")
    let bufSize = 1024 * 1024
    let jsonBuffer = UnsafeMutablePointer<CChar>.allocate(capacity: bufSize)
    defer { jsonBuffer.deallocate() }

    let written = ttzip_rust_bench_run_scenario_matrix(jsonBuffer, bufSize)
    guard written > 0 else {
        print("❌ Scenario benchmark matrix execution failed with status: \(written)")
        exit(1)
    }

    let jsonString = String(cString: jsonBuffer)
    if let jsonPath = jsonOut {
        try? jsonString.write(toFile: jsonPath, atomically: true, encoding: .utf8)
        print("📄 Scenario Telemetry JSON report exported to: \(jsonPath)")
    }
    print("✅ Scenario benchmark matrix completed successfully.")
    exit(0)

case "gate":
    print("⚡️ Running TTZip Automated Benchmark Gate (Rust Native C-ABI)...")
    let status = ttzip_rust_bench_run_gate()
    if status == 0 {
        print("✅ GATE PASSED: All hardware & codec invariants verified.")
        exit(0)
    } else {
        print("❌ GATE FAILED with status code: \(status)")
        exit(70)
    }

default:
    print("❌ Unknown subcommand: '\(command)'\n")
    printHelp()
    exit(64)
}
