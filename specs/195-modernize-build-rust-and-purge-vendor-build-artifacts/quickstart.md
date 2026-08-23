# Quickstart: 195-modernize-build-rust-and-purge-vendor-build-artifacts

## Validation Scenarios

### Scenario 1: Rebuild Rust & Check Direct XCFramework Output
- **Command**: `./scripts/build_rust.sh`
- **Expected Output**: 
  - `Vendor/TTZipVendor.xcframework/macos-arm64/libTTZipVendor.a` updated.
  - No `Vendor/lib` or `Vendor/include` created.

### Scenario 2: Local CI/CD Gate
- **Command**: `./scripts/run_local_ci_gate.sh`
- **Expected Output**: 4/4 stages PASS in $<10\text{s}$.
