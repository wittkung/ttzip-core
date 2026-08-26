# Implementation Plan: 194-purge-dead-services-legacy-vendor-artifacts-and-cmake-relics

## Technical Context
- **Objective**: Purge 8 dead service/utility files from `Sources/TTZipCore/`, delete `Vendor/include/` and `Vendor/lib/`, and remove obsolete `CMakeLists.txt`, `cmake/`, and root `reinstall.sh`.

---

## Constitution Check
- [x] **Zero Cloud Actions Quota**: 100% local validation.
- [x] **Single Source of Truth**: Modern SwiftPM + Cargo architecture.

---

## Phase 0: Research Items
- R001 [SUBAGENT:research] 《TTZipCore 死服务与工具类排查》: Completed.
- R002 [SUBAGENT:research] 《Vendor 与 CMake 历史残渣清理评估》: Completed.

---

## Phase 1: Purge Dead Services & Utilities
- Delete:
  - `Sources/TTZipCore/Services/JSONCoderCache.swift`
  - `Sources/TTZipCore/Services/ArchiveEntryMetadataPool.swift`
  - `Sources/TTZipCore/Services/ArchiveKeyCacheManager.swift`
  - `Sources/TTZipCore/Services/TTZipFinderSyncController.swift`
  - `Sources/TTZipCore/QuickLook/TTZipQuickLookProvider.swift`
  - `Sources/TTZipCore/Utilities/UUDecoder.swift`
  - `Sources/TTZipCore/Utilities/StateBox.swift`
  - `Sources/TTZipCore/Utilities/TTZipProcessExecutor.swift`

## Phase 2: Purge Legacy Vendor Headers & Libraries
- Delete `Vendor/include/` (30 files).
- Delete `Vendor/lib/` (9 files).

## Phase 3: Purge CMake & Duplicate Root Files
- Delete `CMakeLists.txt` and `cmake/`.
- Delete root `reinstall.sh`.

## Phase 4: Final CI Verification
- Run `swift build` and `swift test`.
- Run `cargo test --workspace`.
- Run `./scripts/run_local_ci_gate.sh`.
