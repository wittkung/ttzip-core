# Quickstart Validation: 173-refactor-god-files-and-modular-decomposition

## Scenario 1: Line Count Compliance Verification
- **Command**:
  ```bash
  find Sources rust -type f \( -name "*.swift" -o -name "*.rs" -o -name "CTTZipBridge.c" \) ! -path "*/fast-lzma2/*" ! -path "*/lzfse/*" ! -path "*/zopfli/*" ! -path "*/snappy/*" -exec wc -l {} + | awk '$1 > 500 { print; exit 1 }'
  ```
- **Expected Output**: Exit code 0 (no lines printed, indicating zero first-party files exceed 500 lines).
- **Failure Diagnostic**: If any file prints, inspect the file path and decompose remaining subroutines.

---

## Scenario 2: Swift & Rust Test Matrix Verification
- **Command**:
  ```bash
  swift test && cargo test --manifest-path rust/ttzip-glue/Cargo.toml && cargo test --manifest-path rust/ttzip-tui/Cargo.toml
  ```
- **Expected Output**: All test suites pass with 0 failures and 0 warnings.
- **Failure Diagnostic**: Check compiler errors for broken internal symbols or missing re-exports.

---

## Scenario 3: Full CI Gate Verification
- **Command**:
  ```bash
  ./scripts/run_local_ci_gate.sh
  ```
- **Expected Output**: `Total: 7 Passed, 0 Failed`.
- **Failure Diagnostic**: Check stdout/stderr from failed stage and resolve regressions.
