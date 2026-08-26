# Phase 1 Data Model: Full Codebase Remediation & Safety Entities

**Feature**: `specs/054-codebase-codereview`
**Created**: 2026-08-17
**Status**: Ready

---

## 1. CI Invariant Linter Models (`ci_invariant_linter.schema.json`)

### `LinterScanResult`
- **Fields**:
  - `scanTimestamp` (`string`, ISO-8601 formatted datetime): Execution timestamp of the scan.
  - `totalViolationsCount` (`integer`, minimum 0): Total count of detected code invariant violations.
  - `passed` (`boolean`): True if `totalViolationsCount == 0`, false otherwise.
  - `violations` (`array` of `LinterViolation`): Detailed list of detected violations.
- **Constraints**: `required: ["scanTimestamp", "totalViolationsCount", "passed", "violations"]`

### `LinterViolation`
- **Fields**:
  - `ruleId` (`string`, enum: `["NO_HARDCODED_USERS_PATH", "NO_BARE_PRINT_LOGGING", "NO_HOTPATH_DATA_COUNT", "NO_CONCURRENT_PERFORM_LOCK"]`): Rule identifier.
  - `filePath` (`string`): Project-relative path to offending file.
  - `lineNumber` (`integer`, minimum 1): 1-indexed line number where violation occurs.
  - `snippet` (`string`): Exact code snippet violating the rule.
  - `remediationHint` (`string`): Prescribed fix instruction.
- **Constraints**: `required: ["ruleId", "filePath", "lineNumber", "snippet", "remediationHint"]`

---

## 2. Engine Strategy & Template Coordination Models (`engine_strategy_template.schema.json`)

### `WorkflowExecutionContext`
- **Fields**:
  - `operationId` (`string`, UUID format): Unique workflow run identifier.
  - `operationType` (`string`, enum: `["compress", "extract", "testArchive", "repair"]`): Target operation.
  - `format` (`string`, enum: `["zip", "7z", "tar", "tar.zst", "tar.gz", "tar.bz2", "tar.xz", "lz4", "brotli", "snappy", "wim", "dmg", "iso", "lzip", "lrzip", "aar"]`): Target archive format.
  - `archivePath` (`string`): Path to input/output archive file.
  - `destinationDir` (`string`): Extraction target directory.
  - `isEncrypted` (`boolean`): Whether archive uses password encryption.
  - `hasPassword` (`boolean`): Whether a password was provided.
  - `executionMode` (`string`, enum: `["template_orchestrated", "direct_c_bypass"]`): Selected pipeline driver.
- **Constraints**: `required: ["operationId", "operationType", "format", "archivePath", "destinationDir", "isEncrypted", "hasPassword", "executionMode"]`

### `WorkflowExecutionResult`
- **Fields**:
  - `operationId` (`string`, UUID format): Matches context `operationId`.
  - `isSuccess` (`boolean`): Execution outcome.
  - `bytesProcessed` (`integer`, minimum 0): Uncompressed payload size processed in bytes.
  - `compressedSizeBytes` (`integer`, minimum 0): Final archive size in bytes.
  - `errorCode` (`integer`): 0 for success, negative integer corresponding to `TTZipErrorCode`.
  - `errorMessage` (`string`): Empty on success, descriptive diagnostic on failure.
- **Constraints**: `required: ["operationId", "isSuccess", "bytesProcessed", "compressedSizeBytes", "errorCode", "errorMessage"]`

---

## 3. UI Progress Throttler Event Models (`ui_progress_throttler.schema.json`)

### `ThrottledProgressEvent`
- **Fields**:
  - `fractionCompleted` (`number`, minimum 0.0, maximum 1.0): Current progress percentage.
  - `processedBytes` (`integer`, minimum 0): Bytes written or extracted.
  - `totalBytes` (`integer`, minimum 0): Total target payload bytes.
  - `currentFileName` (`string`): Name of the item currently being compressed/extracted.
  - `isTerminal` (`boolean`): True if `fractionCompleted == 1.0` or operation was cancelled/errored.
  - `timestampUptimeNanoseconds` (`integer`, minimum 0): Monotonic uptime in nanoseconds.
- **Constraints**: `required: ["fractionCompleted", "processedBytes", "totalBytes", "currentFileName", "isTerminal", "timestampUptimeNanoseconds"]`

---

## 4. System Differential Oracle Models (`system_differential_oracle.schema.json`)

### `DifferentialOracleVerification`
- **Fields**:
  - `testCaseName` (`string`): Unique test suite case name.
  - `direction` (`string`, enum: `["ttzip_pack_system_extract", "system_pack_ttzip_extract"]`): Test direction.
  - `format` (`string`, enum: `["zip", "tar", "tar.gz", "tar.bz2", "tar.xz"]`): Archive format.
  - `systemToolPath` (`string`, enum: `["/usr/bin/unzip", "/usr/bin/tar"]`): Native system utility.
  - `payloadSha256Original` (`string`, length 64): SHA-256 hash of original input dataset.
  - `payloadSha256Extracted` (`string`, length 64): SHA-256 hash of verified extracted dataset.
  - `hashesMatch` (`boolean`): `payloadSha256Original == payloadSha256Extracted`.
  - `exitCode` (`integer`): Process exit code of system tool.
- **Constraints**: `required: ["testCaseName", "direction", "format", "systemToolPath", "payloadSha256Original", "payloadSha256Extracted", "hashesMatch", "exitCode"]`
