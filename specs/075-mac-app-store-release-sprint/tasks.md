# Tasks: 075 Mac App Store Release Sprint

**Feature Branch**: `075-mac-app-store-release-sprint`  
**Created**: 2026-08-18  
**Status**: Completed  

---

## Phase 1: Foundational Assets & Metadata

- [x] T001 Implement `scripts/generate_app_icon.sh` generating `Sources/TTZipApp/Resources/AppIcon.icns` from `logo/AppIcon.png`.
- [x] T002 Implement Apple Privacy Manifest `Sources/TTZipApp/PrivacyInfo.xcprivacy`.
- [x] T003 Update `Sources/TTZipApp/TTZip.entitlements` with required App Sandbox keys.
- [x] T004 Expand `Sources/TTZipApp/Info.plist` with full 16-format UTIs and localized bundle names.

---

## Phase 2: MAS Build & Packaging Pipeline

- [x] T005 Implement `scripts/package_mas_app.sh` building `-DMAS_BUILD -c release` and creating `dist/mas/TTZip.app`.
- [x] T006 Implement unit test suite `Tests/TTZipTests/AppStorePackageAuditTests.swift` validating sandbox, entitlements, PrivacyInfo, and UTI completeness.

---

## Phase 3: Verification & Polish

- [x] T007 Run `swift test --filter AppStorePackageAuditTests`.
- [x] T008 Run `scripts/package_mas_app.sh` to package and test the `.app` bundle.
- [x] T009 Run local 6-stage CI gate (`scripts/run_local_ci_gate.sh`).
