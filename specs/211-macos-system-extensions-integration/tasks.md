# Tasks: macOS System Extensions Integration (QuickLook & FinderSync)

**Feature**: `211-macos-system-extensions-integration`  
**Status**: `COMPLETED`  

---

## Tasks

- [x] **Task 1: QuickLook Preview Extension Implementation**
  - [x] Create `Sources/TTZipQuickLook/QuickLookPreviewViewController.swift` implementing `QLPreviewingController` and `WKWebView` rendering.
  - [x] Create `Sources/TTZipQuickLook/Info.plist` with `com.apple.quicklook.preview` extension point and 16 archive UTIs.

- [x] **Task 2: Finder Sync Extension Implementation**
  - [x] Create `Sources/TTZipFinderSync/FinderSync.swift` implementing `FIFinderSync` menu items, badge identifiers, and URL dispatching.
  - [x] Create `Sources/TTZipFinderSync/Info.plist` with `com.apple.FinderSync` extension point.

- [x] **Task 3: URL Scheme & IPC Action Dispatching in TTZipApp**
  - [x] Update `Sources/TTZipApp/Info.plist` with `CFBundleURLTypes` for `ttzip://`.
  - [x] Update `Sources/TTZipApp/TTZipApp.swift` with `.onOpenURL` to trigger instant extract, inspect, and compress actions.

- [x] **Task 4: Build Script & App Bundle Packaging Integration**
  - [x] Create `scripts/build_extensions.sh` to compile `.appex` bundles for QuickLook and FinderSync.
  - [x] Update `scripts/package_local_release.sh` to build and bundle `.appex` in `TTZip.app/Contents/PlugIns/`.

- [x] **Task 5: Verification & Quality Gates**
  - [x] Verify Single-File LOC Defense Gate (532 files scanned, 100% $\le 800\text{ LOC}$).
  - [x] Run `swift test` (133 test cases PASS, 0 failures, 2.8s).
  - [x] Run `./scripts/run_local_ci_gate.sh` (4-stage gate 100% PASS in 12.4s).
  - [x] Package local release (`./scripts/package_local_release.sh --version 1.0.0 --skip-dmg`) and inspect `PlugIns/` directory.
  - [x] Verify deep codesign on `TTZip.app` (`codesign --verify --deep --strict`).
  - [ ] Commit and push to `origin main`.
