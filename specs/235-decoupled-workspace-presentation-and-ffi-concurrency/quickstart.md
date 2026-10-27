# Quickstart: 235 - Decoupled Workspace Presentation, Sandbox RAII Leases, and FFI Concurrency Hardening

> **Target Version**: TTWorkspace v6.3  
> **Status**: Ready for Verification  

---

## 1. Running Unit Tests
```bash
# Swift presentation layer tests
swift test --package-path products/ttzip/apple

# Core microkernel Rust tests
cargo test --manifest-path products/ttzip/app-core/Cargo.toml

# Full hermetic Bazel 8 test suite
bazel test -c opt //products/ttzip/apple:TTZipAppTests
```

## 2. Running Monorepo Architecture Gates
```bash
# 800 LOC Gate
python3 tools/ci/lint_loc_gate.py

# Function length & McCabe complexity
python3 tools/ci/lint_code_quality.py

# Proprietary license audit
python3 tools/ci/license_audit.py
```

## 3. Bundling & Deploying Local Release
```bash
bash products/ttzip/apple/scripts/bundle_app.sh --release --install
```
