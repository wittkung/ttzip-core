# Quickstart: Engineering Governance & Quality Gates

## 1. Verify C-ABI Symbol Parity
```bash
./scripts/verify_cabi_symbols.sh
```
Expected output:
```text
Scanning Sources/CTTZipBridge/include/ttzip_rust_glue.h...
Found 118 exported C-ABI functions.
Scanning Vendor/TTZipVendor.xcframework/macos-arm64/libTTZipVendor.a...
✅ [PASS] 100% Symbol Parity (118/118 symbols verified).
```

## 2. Run Large Volume & Differential Rollback Tests
```bash
swift test --filter LargeVolumeStressTests
```

## 3. Run Sanitizer Diagnostics
```bash
./scripts/run_sanitizers.sh --asan
```
