# Quickstart & Verification Guide: Feature 070 (CLI Test System & Standards Professionalization)

This guide provides executable verification scenarios to validate standards compliance, differential oracle testing, fuzzing robustness, and test telemetry.

---

## Scenario 1: Standards Compliance Suite Execution

**Description**: Verify format standard compliance across all 16 supported formats against official RFC/ISO/POSIX citations.

- **Command**:
  ```bash
  swift test --filter ArchiveStandardsComplianceTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'ArchiveStandardsComplianceTests' started.
  Test Case '-[TTZipTests.ArchiveStandardsComplianceTests testAll16FormatStandardsRegistered]' passed.
  Test Case '-[TTZipTests.ArchiveStandardsComplianceTests testZipExtraFieldCompliance]' passed.
  Test Case '-[TTZipTests.ArchiveStandardsComplianceTests testTarPaxExtendedHeaderCompliance]' passed.
  Test Case '-[TTZipTests.ArchiveStandardsComplianceTests testZstandardRFC8878Compliance]' passed.
  Test Suite 'ArchiveStandardsComplianceTests' passed (0 failures).
  ```
- **Failure Diagnostic**:
  - If a format standard is missing, check `ArchiveFormatStandardRegistry.shared` to ensure all 16 formats are registered with non-empty citation records.

---

## Scenario 2: Differential Oracle Comparison (`DifferentialOracleTests`)

**Description**: Perform bidirectional round-trip archiving and extraction comparing TTZip against macOS standard `/usr/bin/tar` and `/usr/bin/unzip`.

- **Command**:
  ```bash
  swift test --filter DifferentialOracleTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'DifferentialOracleTests' started.
  Test Case '-[TTZipTests.DifferentialOracleTests testTarDifferentialRoundtrip]' passed.
  Test Case '-[TTZipTests.DifferentialOracleTests testZipDifferentialRoundtrip]' passed.
  Test Suite 'DifferentialOracleTests' passed (0 failures).
  ```
- **Failure Diagnostic**:
  - If a payload or permission mismatch occurs, review the 16-byte aligned HexDiff output emitted in the test failure message to identify diverging byte offsets.

---

## Scenario 3: Deterministic Mutation Fuzzing (`ArchiveMutationFuzzTests`)

**Description**: Execute 50+ deterministic mutation iterations (corrupted headers, truncated streams, bad CRCs, Zip Slip traversal paths) and assert clean error codes with zero crashes.

- **Command**:
  ```bash
  swift test --filter ArchiveMutationFuzzTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'ArchiveMutationFuzzTests' started.
  Test Case '-[TTZipTests.ArchiveMutationFuzzTests testDeterministicFuzzingStability]' passed.
  Test Case '-[TTZipTests.ArchiveMutationFuzzTests testZipSlipPathTraversalRejection]' passed.
  Test Case '-[TTZipTests.ArchiveMutationFuzzTests testTruncatedStreamGracefulRecovery]' passed.
  Test Suite 'ArchiveMutationFuzzTests' passed (0 failures).
  ```
- **Failure Diagnostic**:
  - If a crash or ASan violation occurs, check the persisted reproducer `.bin` file in the temporary test sandbox matching the failure seed.

---

## Scenario 4: CLI Test Subcommand with NDJSON Telemetry

**Description**: Run `ttzip-cli test` with `--standard zip` and `--json` to verify machine-readable telemetry stream.

- **Command**:
  ```bash
  swift run ttzip-cli test --standard zip --json
  ```
- **Expected Output**:
  ```json
  {"event":"test_started","format":"zip","test_name":"StandardZipCompliance","timestamp":1723907530.0}
  {"event":"test_passed","bytes_processed":1048576,"duration_ms":4.2,"throughput_mbs":249.6,"timestamp":1723907530.004}
  {"event":"session_summary","session_summary":{"failed":0,"passed":1,"skipped":0,"total_duration_ms":4.2,"total_tests":1},"timestamp":1723907530.005}
  ```
- **Failure Diagnostic**:
  - If NDJSON parsing fails, verify that `TerminalRenderEngine.shared.emitNDJSON` formats valid JSON conforming to `contracts/test_telemetry.json`.
