# Feature Specification: 075 Mac App Store Release Sprint (MAS Production Readiness)

## 1. Overview & Business Objectives

This sprint delivers end-to-end production readiness for **Mac App Store (MAS) submission** and independent notarized distribution of TTZip for macOS 14+ (Sonoma) and macOS 15+ (Sequoia).

### Core Goals:
1. **100% App Store Review Guidelines Compliance**:
   - Hard App Sandbox enforcement (`com.apple.security.app-sandbox`) with security-scoped bookmarks and user-selected file read/write permissions.
   - Zero non-sandboxed system calls, zero unauthorized subprocess spawns, and strict conditional separation (`#if !MAS_BUILD`) of third-party update frameworks (Sparkle).
2. **Apple Mandatory Privacy Manifest (`PrivacyInfo.xcprivacy`)**:
   - Explicit declaration of zero tracking (`NSPrivacyTracking: false`), zero collected data types (`NSPrivacyCollectedDataTypes: []`), and necessary file timestamp API access reasons (`NSPrivacyAccessedAPITypes`).
3. **Complete 16-Format UTI & Document Role Bindings**:
   - Comprehensive `Info.plist` declaring uniform type identifiers for ZIP, 7Z, TAR, GZ, BZ2, XZ, ZSTD, LZ4, LZIP, LRZIP, AAR, SNAPPY, WIM, DMG, ISO, RAR, and split archives (`.001`).
4. **Retina AppIcon Assets (`AppIcon.icns`)**:
   - Multi-resolution Apple ICNS icon bundle generated from the master 1024×1024 Retina icon (`logo/AppIcon.png`).
5. **One-Click Automated MAS Packaging & Signing Pipeline (`scripts/package_mas_app.sh`)**:
   - Produces a valid, sandboxed `.app` bundle and signed `.pkg` installer ready for Transporter / App Store Connect submission.

---

## 2. Technical Architecture & Constraints

### A. Sandbox Entitlements Matrix

| Entitlement Key | Value | Purpose |
| :--- | :--- | :--- |
| `com.apple.security.app-sandbox` | `true` | App Store mandatory isolation sandbox |
| `com.apple.security.files.user-selected.read-write` | `true` | Read/write access to user-selected archive files and extraction target directories |
| `com.apple.security.files.bookmarks.app-scope` | `true` | Persistent security-scoped URL bookmarks for pinned/favorite directories |
| `com.apple.security.files.downloads.read-write` | `true` | Seamless extraction to ~/Downloads folder |
| `com.apple.security.network.client` | `true` | Required by macOS out-of-process `WKWebView` XPC rendering service for local HTML/EPUB archive previews |

### B. Cross-Process File Access for `ttarchive-worker` (Public API Compliance — Guideline 2.5.1)

- `Contents/Helpers/ttarchive-worker` runs with `com.apple.security.app-sandbox = true` and `com.apple.security.inherit = true` in `mas` and `steam` channels (and Hardened Runtime without App Sandbox in `direct`).
- Because `com.apple.security.inherit` does not automatically transfer runtime Powerbox (`NSOpenPanel`) file access to child processes, and App Store Review Guideline 2.5.1 prohibits private `sandbox_extension_*` C symbols:
  1. **Public Implicit Security-Scoped URL Bookmarks**: `QuarantineWorkerClient` creates transient security-scoped bookmarks with implicit security scope (`URL.BookmarkCreationOptions.minimalBookmark` / `CFURLCreateBookmarkData` with options `0`) for the user-authorized source archive URL and external destination directory URL, passing them as base64 arguments (`--archive-bookmark`, `--dest-bookmark`) to `ttarchive-worker`.
  2. **Worker Foundation/CoreFoundation Resolution & RAII Lifecycle**: `ttarchive-worker` resolves each bookmark via a lightweight CoreFoundation/Foundation bridge (`CFURLCreateByResolvingBookmarkData` with `kCFURLBookmarkResolutionWithoutUIMask`) and calls `CFURLStartAccessingSecurityScopedResource` on entry and `CFURLStopAccessingSecurityScopedResource` on RAII drop.
  3. **Single-Root-FD Jail Defense-in-Depth Retained**: `QuarantineWorkerClient` duplicates `archiveFd` and `dirFd` above descriptor `10` (`F_DUPFD_CLOEXEC`) and maps them to child FDs `4` (`--archive-fd 4`) and `5` (`--target-dir-fd 5`), preserving `DirJail` `openat`/`mkdirat` traversal confinement.

### C. Per-Component Privacy Manifests (`PrivacyInfo.xcprivacy`)

- Packaged per target according to actual linked symbols and API call sites:
  - `TTZip.app/Contents/Resources/PrivacyInfo.xcprivacy`: `FileTimestamp` (`C617.1`, `3B52.1`), `UserDefaults` (`CA92.1`, `1C8F.1`), `DiskSpace` (`E174.1`, `85F4.1`), `SystemBootTime` (`35F9.1`).
  - `TTZipFinderSync.appex/Contents/Resources/PrivacyInfo.xcprivacy`: `UserDefaults` (`1C8F.1`, `CA92.1`), `FileTimestamp` (`C617.1`, `3B52.1`), `DiskSpace` (`E174.1`), `SystemBootTime` (`35F9.1`).
  - `TTZipQuickLook.appex/Contents/Resources/PrivacyInfo.xcprivacy`: `UserDefaults` (`1C8F.1`, `CA92.1`), `FileTimestamp` (`C617.1`), `DiskSpace` (`E174.1`), `SystemBootTime` (`35F9.1`).
  - `TTMediaWorker.xpc/Contents/Resources/PrivacyInfo.xcprivacy`: `FileTimestamp` (`C617.1`), `SystemBootTime` (`35F9.1`).

---

## 3. Current Release & Acceptance Status

- **Current Status**: `ad-hoc 渠道包及本地产物一致性检查完成`（正式签名、独立签名包集成、公证和商店验收仍未完成）。
- **Verification Requirements**:
  1. `products/ttzip/apple/scripts/build_bazel_channels.sh all` packages isolated `mas`, `direct`, and `steam` `.app` bundles and `.zip` archives with cryptographic source snapshots and SHA-256 manifests.
  2. `products/ttzip/apple/scripts/verify_nsopenpanel_sandbox_host.py` verifies real App-Sandboxed `NSOpenPanel` external archive & external destination selection through the production `QuarantineWorkerClient` bookmark + FD pipeline (`products/ttzip/apple/scripts/verify_synthetic_sandbox_extension.py` is retained separately as the synthetic sandbox extension diagnostic test).
  3. `products/ttzip/core/scripts/generate_acknowledgements.py` dynamically harvests the actual build dependency closure per channel (`mas`, `direct`, `steam`) and embeds full upstream `LICENSE`/`COPYING`/`NOTICE` texts, failing fast if any version or license text is missing.
  4. All monorepo test targets in the release gate (including `//products/ttzip/apple:UIPreviewKitTests`) must pass before final release sign-off.

