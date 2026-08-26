# Feature Specification: 195-modernize-build-rust-and-purge-vendor-build-artifacts

## 1. Executive Summary & Strategic Motivation
1. Modernize `scripts/build_rust.sh` to eliminate redundant steps that re-created `Vendor/lib/`, `Vendor/include/`, and `Vendor/libTTZipVendor.a`.
2. Output Universal static library slices directly to `Vendor/TTZipVendor.xcframework/macos-arm64/libTTZipVendor.a`.
3. Purge redundant `Vendor/libTTZipVendor.a` (22.5 MB).
4. Purge > 516 MB of old CMake build artifacts inside upstream vendor submodules (`Vendor/*/build*`).

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Clean Vendor Build Workflow
- **Given** building Rust glue via `./scripts/build_rust.sh`
- **When** the build completes
- **Then** artifacts are written directly into `Vendor/TTZipVendor.xcframework` and `Sources/CTTZipBridge/include/`.
- **And** no dead directories (`Vendor/lib`, `Vendor/include`) or root `.a` files are created.

### User Scenario 2: Reclaim > 500 MB Disk Space
- **Given** browsing the repository
- **When** examining `Vendor/`
- **Then** all legacy `build/` directories across upstream packages are purged.

---

## 3. Success Metrics
1. `scripts/build_rust.sh` outputs directly to `TTZipVendor.xcframework`.
2. Purge `Vendor/libTTZipVendor.a`.
3. Purge > 516 MB of upstream CMake build artifacts.
4. Pass 4-stage local CI gate in $< 10\text{s}$.
