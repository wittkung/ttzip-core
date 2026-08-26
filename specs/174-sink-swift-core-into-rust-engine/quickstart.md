# Quickstart Validation: 174-sink-swift-core-into-rust-engine

## Scenario 1: Rust Standards & Sniffing Native Verification
- **Command**:
  ```bash
  cargo test --manifest-path rust/ttzip-glue/Cargo.toml -- standards
  ```
- **Expected Output**: All standards compliance and magic sniffer unit tests pass with 0 failures.
- **Failure Diagnostic**: Check magic byte offsets or header length assertions.

---

## Scenario 2: Rust Crypto, Zeroize & RS-FEC Verification
- **Command**:
  ```bash
  cargo test --manifest-path rust/ttzip-glue/Cargo.toml -- crypto
  ```
- **Expected Output**: All ZipCrypto, AES-CTR/CBC, 7z KDF, and Reed-Solomon FEC tests pass with 0 failures.
- **Failure Diagnostic**: Verify Galois Field GF(2^8) arithmetic and Cauchy matrix inversion steps.

---

## Scenario 3: End-to-End Swift & Full CI Gate Verification
- **Command**:
  ```bash
  ./scripts/run_local_ci_gate.sh
  ```
- **Expected Output**: `Total: 7 Passed, 0 Failed`.
- **Failure Diagnostic**: Inspect test logs for failed stages and resolve regressions.
