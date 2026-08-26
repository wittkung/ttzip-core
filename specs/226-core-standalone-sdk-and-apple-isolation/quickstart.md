# Quickstart Guide: Building & Verifying Standalone TTZipCore SDK

**Feature**: `226-core-standalone-sdk-and-apple-isolation`  
**Date**: 2026-08-26

---

## 1. Build the Universal TTZipCore SDK

From `core/`:
```bash
cd /Users/kevintung/Documents/dev/products/ttzip/core

# Execute the automated universal SDK packaging script
./scripts/build_sdk_framework.sh --release
```

This generates:
- `Vendor/TTZipVendor.xcframework` (Universal macOS `arm64` + `x86_64`)
- `dist/TTZipVendor.xcframework.zip`
- `dist/TTZipVendor.xcframework.zip.sha256`

---

## 2. Verify Multi-Architecture Slices

```bash
lipo -info Vendor/TTZipVendor.xcframework/macos-arm64_x86_64/libTTZipVendor.a
# Expected output: Architectures in the fat file: ... are: x86_64 arm64
```

---

## 3. Verify SPM Checksum Calculation

```bash
swift package compute-checksum dist/TTZipVendor.xcframework.zip
```

---

## 4. Build Client Without Rust Toolchain

From `apple/`:
```bash
cd /Users/kevintung/Documents/dev/products/ttzip/apple

# Compile standalone client application
swift build -c release

# Run all 170 unit tests
swift test

# Build bundled macOS app
./scripts/bundle_app.sh --channel direct
```
