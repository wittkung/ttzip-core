# Phase 0 Research: 195-modernize-build-rust-and-purge-vendor-build-artifacts

## Research Item R001: Modernized build_rust.sh Architecture
- **Decision**: 
  - Update `scripts/build_rust.sh` to compile `ttzip-glue` and lipo/strip directly into `${VENDOR_DIR}/TTZipVendor.xcframework/macos-arm64/libTTZipVendor.a`.
  - Remove all logic creating `${VENDOR_DIR}/lib` and `${VENDOR_DIR}/include`.
- **Rationale**: 
  - `Package.swift` only consumes `TTZipVendor.xcframework`.
- **Alternatives Considered**: 
  - *Keep legacy lib directory*: Re-introduces deleted dead directories on every rust rebuild.
- **Source**: 
  - `scripts/build_rust.sh`
  - `Package.swift`

---

## Research Item R002: Upstream CMake Artifact Purge
- **Decision**: 
  - Delete all `build/` and `build_*/` directories within `Vendor/turbobench`, `Vendor/zlib-ng-upstream`, `Vendor/libarchive-upstream`, `Vendor/xz-upstream`, `Vendor/zstd-upstream`, `Vendor/lz4-upstream`.
- **Rationale**: 
  - Frees > 516 MB of untracked binary and object file bloat.
- **Alternatives Considered**: 
  - *Keep upstream build dirs*: Wasteful and causes clutter during file searches.
- **Source**: 
  - `Vendor/*/build*`
