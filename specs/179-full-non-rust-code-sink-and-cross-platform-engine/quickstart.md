# Quickstart: 179-full-non-rust-code-sink-and-cross-platform-engine

## Validation Scenarios

### Scenario 1: Rust Unit & Integration Tests Verification
- **Command**: `cargo test --workspace`
- **Expected Output**: All unit tests pass across `ttzip-glue` and `ttzip-tui`.
- **Failure Diagnostic**: Inspect any assertion failures in `security::path_sanitizer`, `charset`, `crypto::rs_fec`, `fs::scanner`, or `testing::hex_diff`.

### Scenario 2: Swift 872+ Suite & C-ABI Integration
- **Command**: `swift test`
- **Expected Output**: Executed 872+ tests with 0 failures and 0 warnings.
- **Failure Diagnostic**: Verify C-ABI symbols in `Sources/CTTZipBridge/include/ttzip_rust_glue.h` match exported `#[no_mangle]` Rust functions in `Vendor/libTTZipVendor.a`.

### Scenario 3: Local CI/CD Automated Regression & Performance Gate
- **Command**: `./scripts/run_local_ci_gate.sh`
- **Expected Output**: 7/7 stages PASS (Unit, Compliance, Differential Oracle, Mutation Fuzz, Libarchive Corpus, Deflate Bench, Rust Industrial).
- **Failure Diagnostic**: Check individual stage logs in standard output for regression details.
