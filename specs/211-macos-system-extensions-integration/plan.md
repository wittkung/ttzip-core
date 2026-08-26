# Implementation Plan: macOS System Extensions Integration

**Feature**: `211-macos-system-extensions-integration`  
**Status**: `IN_PROGRESS`  

---

## 1. Technical Architecture

```
TTZip.app/Contents/
├── MacOS/
│   └── TTZip (Main SwiftUI Application)
├── Helpers/
│   └── ttzip (Standalone Rust CLI / IPC Engine)
├── Frameworks/
│   ├── Sparkle.framework
│   └── (Dynamic Libraries)
└── PlugIns/
    ├── TTZipQuickLook.appex (Spacebar Preview Extension)
    └── TTZipFinderSync.appex (Finder Context Menu Extension)
```

---

## 2. Modules to Implement

1. **`Sources/TTZipQuickLook/`**:
   - `QuickLookPreviewViewController.swift`: Implements `QLPreviewingController` and `WKNavigationDelegate`, renders HTML preview using `QuickLookPreviewEngine`.
   - `Info.plist`: Declares `com.apple.quicklook.preview` extension point and UTI format associations for all 16 supported archive formats.
2. **`Sources/TTZipFinderSync/`**:
   - `FinderSync.swift`: Subclasses `FIFinderSync`, tracks monitored folders, sets badges, constructs dynamic menus via `FinderSyncHelper`, and dispatches requests.
   - `Info.plist`: Declares `com.apple.FinderSync` extension point.
3. **`Sources/TTZipApp/` Custom URL Scheme Handler**:
   - Register `ttzip` URL scheme in `Sources/TTZipApp/Info.plist`.
   - Handle `ttzip://extract?path=...`, `ttzip://inspect?path=...`, `ttzip://compress?path=...` in `TTZipApp.swift`.
4. **Build & Release Packaging Automation**:
   - Script `scripts/build_extensions.sh` to compile extension bundles.
   - Integrate into `scripts/package_local_release.sh` step 3 (`assemble_app_bundle`).
