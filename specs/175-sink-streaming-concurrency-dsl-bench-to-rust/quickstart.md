# Quickstart Validation: 175-sink-streaming-concurrency-dsl-bench-to-rust

## Scenario 1: 7z Solid Stream & Zstd Pipeline Verification
- **Command**:
  ```bash
  cargo test --manifest-path rust/ttzip-glue/Cargo.toml -- sevenz codecs::zstd
  ```
- **Expected Output**: Solid stream extraction and Zstd bounded stream tests pass with 0 failures.
- **Failure Diagnostic**: Verify 7z Folder early termination bounds and Zstd CCtx/DCtx stream flush return codes.

---

## Scenario 2: Lock-Free Ring Buffer, DSL & Pareto Verification
- **Command**:
  ```bash
  cargo test --manifest-path rust/ttzip-glue/Cargo.toml -- runtime::ring_buffer fs::filter bench::pareto
  ```
- **Expected Output**: SPSC/MPMC lock-free tests, DSL filter evaluations, and Andrew's Monotone Chain 2D convex hull tests pass.
- **Failure Diagnostic**: Inspect queue wrap-around indices and 2D cross-product collinear tolerance.

---

## Scenario 3: Full Workspace Regression & Local CI Gate
- **Command**:
  ```bash
  ./scripts/run_local_ci_gate.sh
  ```
- **Expected Output**: `Total: 7 Passed, 0 Failed`.
- **Failure Diagnostic**: Review stage logs and resolve any Swift-Rust FFI parameter mismatches.
