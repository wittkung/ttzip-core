# Phase 1 Data Model: Quality Governance & Compliance Schema

**Feature**: `220-comprehensive-codebase-and-quality-audit`  
**Date**: 2026-08-24  
**Status**: Completed  

---

## 1. Entities & Data Structures

### 1.1 QualityAuditReport
Represents the global repository audit state across files, lines of code, and subsystem distributions.

| Field | Type | Description | Validation Rule |
| :--- | :--- | :--- | :--- |
| `scanned_timestamp` | String (ISO 8601) | Audit generation timestamp | Must be valid UTC datetime |
| `total_files` | UInt32 | Total source files in repository | Must equal sum of language file counts |
| `total_loc` | UInt64 | Total physical lines of code | Must equal sum of language LOC |
| `loc_gate_passed` | Boolean | True if all files <= 800 LOC | Hard invariant |
| `standards_latch_passed` | Boolean | True if SPDX & ASCII checks pass | Hard invariant |
| `modules` | Array<ModuleMetric> | Per-module breakdown | Non-empty |

### 1.2 LicenseComplianceRecord
Records the license header status of an individual source file.

| Field | Type | Description | Validation Rule |
| :--- | :--- | :--- | :--- |
| `file_path` | String | Relative path from repo root | File must exist on disk |
| `has_spdx_tag` | Boolean | True if SPDX identifier present | Must be true for TTZip source |
| `spdx_identifier` | String | `BSD-3-Clause OR Apache-2.0` | Fixed string contract |
| `is_ascii_clean` | Boolean | True if zero non-ASCII bytes in C headers | Must be true for C Bridge |

### 1.3 SdkTestMatrixResult
Records test execution results for client SDK runtimes.

| Field | Type | Description | Values |
| :--- | :--- | :--- | :--- |
| `language` | String | Target language binding | `c`, `cpp`, `python`, `node`, `dart`, `jvm`, `dotnet` |
| `toolchain_available` | Boolean | Whether runtime binary exists | `true` / `false` |
| `status` | String | Execution outcome | `passed`, `failed`, `skipped` |
| `duration_ms` | UInt32 | Test execution time | $\ge 0$ |
| `diagnostic_output` | String | Error or stdout telemetry | Truncated to 4KB if failed |

### 1.4 CiGateStageResult
Records individual stage execution within `run_local_ci_gate.sh`.

| Field | Type | Description |
| :--- | :--- | :--- |
| `stageIndex` | UInt32 (1..4) | Stage sequence index |
| `name` | String | Human readable stage description |
| `command` | String | Shell execution string |
| `status` | String (`pass`, `fail`, `skip`) | Execution result |
| `durationSeconds` | Float64 | Elapsed wall-clock time |
| `diagnosticMessage` | Optional<String> | Failure output if non-zero exit |
