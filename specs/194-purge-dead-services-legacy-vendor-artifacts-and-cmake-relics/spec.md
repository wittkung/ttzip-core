# Feature Specification: 194-purge-dead-services-legacy-vendor-artifacts-and-cmake-relics

## 1. Executive Summary & Strategic Motivation
A thorough codebase scan discovered 4 categories of dead code and historical build debris:
1. **Dead Services & Utilities**: 6 unreferenced Swift files in `Sources/TTZipCore/Services/` and `Utilities/` (~516 LOC).
2. **Dead Desktop Integration Shells**: 2 unreferenced files in `Services/` and `QuickLook/` (~250 LOC).
3. **Legacy Vendor Residue**: 39 obsolete files in `Vendor/include/` and `Vendor/lib/`.
4. **Obsolete CMake & Duplicate Relics**: `CMakeLists.txt` (318 LOC), `cmake/`, and duplicate `reinstall.sh` in the root directory.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Lean & Pure TTZipCore
- **Given** browsing `Sources/TTZipCore/`
- **When** checking services and utilities
- **Then** every single file is actively imported and tested by core workflows or higher layers.

### User Scenario 2: Single Source of Truth Build System
- **Given** building and developing TTZip
- **When** checking the root workspace
- **Then** only modern SwiftPM (`Package.swift`) and Cargo (`rust/Cargo.toml`) build definitions exist.

---

## 3. Success Metrics
1. Delete 8 unreferenced Swift files from `Sources/TTZipCore/`.
2. Delete `Vendor/include/` and `Vendor/lib/`.
3. Delete `CMakeLists.txt`, `cmake/`, and root `reinstall.sh`.
4. Pass all automated tests and 4-stage local CI gates.
