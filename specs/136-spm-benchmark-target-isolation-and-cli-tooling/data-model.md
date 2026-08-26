# Data Model: SPM Benchmark Target Isolation & `ttzip-bench` CLI Tooling

**Feature Directory**: `specs/136-spm-benchmark-target-isolation-and-cli-tooling`  
**Status**: Approved  

---

## 1. Entities & Data Models

### 1.1 `BenchmarkCliCommand`
Represents the requested sub-action dispatched by `ttzip-bench`.

- **`commandName`**: `String` (`"matrix"`, `"plot"`, `"gate"`, `"help"`)
- **`jsonOutputPath`**: `String?` (optional destination for report JSON)
- **`engineFilter`**: `String?` (optional codec filter)
- **`gateThresholdCv`**: `Double` (max acceptable CV %, default `1.50`)
- **`gateMaxRegressionPct`**: `Double` (max acceptable throughput regression %, default `2.00`)

### 1.2 `BenchmarkCliReport`
Structured benchmark telemetry output adhering to `contracts/bench_cli_matrix_schema.json`.

- **`timestamp`**: `Int64` (epoch timestamp in seconds)
- **`osVersion`**: `String` (e.g. `"macOS 15.3"`)
- **`architecture`**: `String` (e.g. `"arm64"`)
- **`totalPoints`**: `Int` (number of executed points, e.g. `50`)
- **`passedPoints`**: `Int` (number of passing points)
- **`totalDurationSeconds`**: `Double` (execution duration)
- **`medianCvPercentage`**: `Double` (median CV %)
- **`gateVerdict`**: `String` (`"PASS"`, `"FAIL"`)
- **`points`**: `[BenchmarkPointTelemetry]` (array of individual point results)
