# Phase 0 Research: 194-purge-dead-services-legacy-vendor-artifacts-and-cmake-relics

## Research Item R001: TTZipCore Dead File Audit
- **Decision**: 
  - Delete `JSONCoderCache.swift`, `ArchiveEntryMetadataPool.swift`, `ArchiveKeyCacheManager.swift`, `UUDecoder.swift`, `StateBox.swift`, `TTZipProcessExecutor.swift`, `TTZipFinderSyncController.swift`, `TTZipQuickLookProvider.swift`.
- **Rationale**: 
  - Each of these 8 files has exactly 0 usages across `Sources/` and `Tests/`.
- **Alternatives Considered**: 
  - *Keep them*: Adds dead code maintenance debt.
- **Source**: 
  - Repository-wide grep audit across `Sources/` and `Tests/`.

---

## Research Item R002: Legacy Vendor & CMake Cleanup
- **Decision**: 
  - Delete `Vendor/include/`, `Vendor/lib/`, `CMakeLists.txt`, `cmake/`, and root `reinstall.sh`.
- **Rationale**: 
  - SwiftPM relies exclusively on `Vendor/TTZipVendor.xcframework` (built by `scripts/build_rust.sh`).
  - CMake was superseded by SwiftPM + Cargo.
  - Root `reinstall.sh` is an exact duplicate of `scripts/reinstall.sh`.
- **Alternatives Considered**: 
  - *Keep CMakeLists.txt*: Misleads developers into believing CMake is a supported build system.
- **Source**: 
  - `Package.swift`
  - `scripts/build_rust.sh`
  - `scripts/reinstall.sh`
