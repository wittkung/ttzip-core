# Data Model: Test Suite Architecture Governance, Target Isolation & Unified Corpus Infrastructure

**Feature Directory**: `specs/135-test-suite-governance-and-target-isolation`  
**Status**: Approved  

---

## 1. Entities & Data Models

### 1.1 `BenchmarkCorpusDescriptor`
Describes a deterministic corpus payload used across micro and macro benchmarks.

- **`corpusId`**: `String` (e.g., `"text"`, `"striped_rgb"`, `"dna"`, `"mixed"`)
- **`displayName`**: `String` (e.g., `"Source Code & English Text"`)
- **`sizeBytes`**: `Int` (byte length, e.g., `131072`, `1048576`, `104857600`)
- **`entropyEstimate`**: `Double` (Shannon entropy estimation $0.0 \sim 8.0$)
- **`sha256Fingerprint`**: `String` (64-character hexadecimal SHA-256 hash)

### 1.2 `SystemBinaryDescriptor`
Represents an external system utility resolved dynamically across environments.

- **`binaryName`**: `String` (e.g., `"zstd"`, `"7zz"`, `"bsdtar"`, `"unzip"`, `"pigz"`, `"advzip"`)
- **`resolvedPath`**: `String` (absolute path, e.g., `"/opt/homebrew/bin/zstd"`)
- **`resolutionSource`**: `String` (`"environment"`, `"bundle"`, `"path"`, `"standard_directory"`)
- **`isAvailable`**: `Bool` (whether the binary executable exists and is runnable)
- **`versionString`**: `String` (e.g., `"zstd v1.5.6"`, `"7-Zip 24.05"`)

### 1.3 `BenchmarkMatrixExecutionReport`
Full telemetry output from the 50-point in-memory matrix and CI audit gate.

- **`timestamp`**: `Int64` (epoch seconds)
- **`totalPoints`**: `Int` (number of executed benchmark points, fixed 50)
- **`passedPoints`**: `Int` (number of passing points)
- **`totalDurationMs`**: `Double` (overall matrix execution duration in milliseconds)
- **`medianCvPercentage`**: `Double` (median coefficient of variation percentage)
- **`regressionPointsCount`**: `Int` (number of points regressing $> 2.0\%$)
- **`gateVerdict`**: `String` (`"PASS"`, `"WARN"`, `"BLOCKED"`)
