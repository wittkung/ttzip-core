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
    OVERVIEW: TTZip Benchmark, Architecture Gate & Telemetry CLI (Pure UniFFI & Apple Silicon Native)

    USAGE: ttzip-bench <subcommand> [options]

    SUBCOMMANDS:
      ab              Run declarative A/B benchmark suite with Welch's t-test statistical inference
      ffi-tax         Run Swift 6 -> Rust UniFFI cross-language FFI tax & zero-copy latency benchmark
      scenario-matrix Run enterprise 100-scenario high-pressure benchmark matrix (7 industrial categories)
      matrix-gate     Run full matrix benchmark across 8 mathematical synthetic corpus types & codecs
      scenario-gate   Run real-world scenario gates (Office, Code, Media, Entropy, Deep VFS)
      mips-score      Calculate Apple Silicon MIPS rating, memory bandwidth & hardware efficiency index
      ab-sample       Run differential A/B benchmark comparing baseline vs hardware-accelerated kernels
      pipeline        Execute full-pipeline E2E benchmark (Swift Facade -> UniFFI -> Rust -> APFS I/O)
      gate            Run automated regression and hardware stability checks for CI/CD
      help            Display this help message

    A/B BENCHMARK OPTIONS (for 'ab' subcommand):
      --target <glob>        Wildcard filter pattern matching target URIs (default: "*")
      --corpus <uri>         Corpus URI identifier (default: "synthetic:zipf_text")
      --size <bytes>         Corpus payload size in bytes (default: 1048576)
      --baseline-json <path> Path to offline baseline JSON snapshot for differential regression check
      --snapshot-out <path>  Path to export canonical golden baseline snapshot JSON (Milestone 6)
      --rounds <n>           Number of timed measurement iterations per target (default: 10)
      --format <fmt>         Output format: table (ASCII table), json (RFC 8259), markdown (GitHub PR)

    GLOBAL OPTIONS:
      --json-out <path>      Write structured telemetry report to JSON file
      --lang <bcp47>         Force target language (en, zh-Hans, zh-Hant, ja, de, fr, es)
      --size-mb <int>        Set corpus payload size in MB (default: 10)
    """)
}

// MARK: - Entry Point

let args = Array(CommandLine.arguments.dropFirst())
guard let command = args.first, command != "--help", command != "-h", command != "help" else {
    printHelp()
    exit(0)
}

var jsonOut: String?
var snapshotOut: String?
var corpusSizeMB = 10
var targetFilter = "*"
var corpusUri = "synthetic:zipf_text"
var customSizeBytes: UInt64?
var baselineJsonPath: String?
var rounds: UInt32 = 10
var outputFormat = "table"

var idx = 1
while idx < args.count {
    let arg = args[idx]
    if (arg == "--json-out" || arg == "-o"), idx + 1 < args.count {
        jsonOut = args[idx + 1]
        idx += 2
    } else if (arg == "--snapshot-out" || arg == "-sout"), idx + 1 < args.count {
        snapshotOut = args[idx + 1]
        idx += 2
    } else if arg == "--lang", idx + 1 < args.count {
        let langStr = args[idx + 1]
        if let parsed = AppLanguage.from(identifier: langStr) {
            TTZipLocalizationManager.shared.currentLanguage = parsed
        }
        idx += 2
    } else if arg == "--size-mb", idx + 1 < args.count {
        corpusSizeMB = Int(args[idx + 1]) ?? 10
        idx += 2
    } else if (arg == "--target" || arg == "-t"), idx + 1 < args.count {
        targetFilter = args[idx + 1]
        idx += 2
    } else if (arg == "--corpus" || arg == "-c"), idx + 1 < args.count {
        corpusUri = args[idx + 1]
        idx += 2
    } else if (arg == "--size" || arg == "-s"), idx + 1 < args.count {
        customSizeBytes = UInt64(args[idx + 1])
        idx += 2
    } else if (arg == "--baseline-json" || arg == "-b"), idx + 1 < args.count {
        baselineJsonPath = args[idx + 1]
        idx += 2
    } else if (arg == "--rounds" || arg == "-r" || arg == "-n"), idx + 1 < args.count {
        rounds = UInt32(args[idx + 1]) ?? 10
        idx += 2
    } else if (arg == "--format" || arg == "-f"), idx + 1 < args.count {
        outputFormat = args[idx + 1]
        idx += 2
    } else {
        idx += 1
    }
}

switch command {
case "ab":
    let effectiveSize = customSizeBytes ?? (args.contains("--size-mb") ? UInt64(corpusSizeMB * 1024 * 1024) : 1048576)
    let success = executeAbBenchmark(
        targetFilter: targetFilter,
        corpusUri: corpusUri,
        sizeBytes: effectiveSize,
        baselineJsonPath: baselineJsonPath,
        rounds: rounds,
        format: outputFormat,
        jsonOut: jsonOut,
        snapshotOut: snapshotOut
    )
    exit(success ? 0 : 70)

case "ffi-tax":
    let success = executeFfiTaxBenchmark(jsonOut: jsonOut)
    exit(success ? 0 : 70)

case "scenario-matrix", "scenario-gate":
    let success = execute100ScenarioMatrix(jsonOut: jsonOut)
    exit(success ? 0 : 70)

case "matrix-gate":
    let success = executeMatrixGate(corpusSizeMB: corpusSizeMB, jsonOut: jsonOut)
    exit(success ? 0 : 70)

case "mips-score":
    executeMipsScore(jsonOut: jsonOut)
    exit(0)

case "ab-sample":
    executeAbSample(jsonOut: jsonOut)
    exit(0)

case "pipeline":
    executePipelineBenchmark(jsonOut: jsonOut)
    print("✅ Pipeline benchmark completed successfully.")
    exit(0)

case "gate":
    let success = executeGateBenchmark(jsonOut: jsonOut)
    exit(success ? 0 : 70)

default:
    print("❌ Unknown subcommand: '\(command)'\n")
    printHelp()
    exit(64)
}
