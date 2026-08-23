# Quickstart Validation: 176-sink-snappy-brotli-split-entropy-recovery-to-rust

## Scenario 1: Snappy Framing & Brotli Pipeline Verification
- **Command**:
  ```bash
  cargo test --manifest-path rust/ttzip-glue/Cargo.toml -- codecs::snappy codecs::brotli
  ```
- **Expected Output**: Standard CRC32-C Snappy Framing and pure Rust Brotli streaming tests pass with 0 failures.
- **Failure Diagnostic**: Verify `snap` FrameDecoder header assertions and `brotli` parameter mappings.

---

## Scenario 2: Multi-Volume, SIMD Entropy, VFS Cache & In-Memory Recovery
- **Command**:
  ```bash
  cargo test --manifest-path rust/ttzip-glue/Cargo.toml -- archive::split analytics::entropy vfs::cache_pool crypto::recovery
  ```
- **Expected Output**: Multi-volume rotation, SIMD histogram reduction, $O(1)$ LRU eviction, and in-memory password tests pass.
- **Failure Diagnostic**: Inspect NEON reduction register boundaries and PVV short-circuit logic.

---

## Scenario 3: Full Workspace Regression & Local CI Gate
- **Command**:
  ```bash
  ./scripts/run_local_ci_gate.sh
  ```
- **Expected Output**: `Total: 7 Passed, 0 Failed`.
- **Failure Diagnostic**: Review stage logs and resolve any Swift-Rust FFI parameter mismatches.
