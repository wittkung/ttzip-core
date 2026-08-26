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
| `com.apple.security.network.client` | `false` (MAS) | Zero network access in MAS build (100% local, offline, privacy-first) |

### B. Privacy Manifest (`PrivacyInfo.xcprivacy`)

- `NSPrivacyTracking`: `false`
- `NSPrivacyCollectedDataTypes`: `[]`
- `NSPrivacyAccessedAPITypes`:
  - `NSPrivacyAccessedAPICategoryFileTimestamp`: Reason `C617.1` (Accessing file timestamps inside archives to preserve creation/modification dates during compression and extraction).

### C. Bundle Structure

```
TTZip.app/
├── Contents/
│   ├── MacOS/
│   │   └── TTZip (Universal 2 or arm64/x86_64 stripped binary)
│   ├── Resources/
│   │   ├── AppIcon.icns
│   │   ├── PrivacyInfo.xcprivacy
│   │   ├── en.lproj/InfoPlist.strings
│   │   └── zh_CN.lproj/InfoPlist.strings
│   ├── Info.plist
│   └── PkgInfo
```

---

## 3. Success Criteria

1. **Gate 1**: `swift build -c release -Xswiftc -DMAS_BUILD` compiles cleanly with zero warnings or errors.
2. **Gate 2**: `scripts/package_mas_app.sh` packages `TTZip.app`, embeds `PrivacyInfo.xcprivacy`, `AppIcon.icns`, signs with entitlements, and verifies via `codesign -dvvv`.
3. **Gate 3**: `PrivacyInfo.xcprivacy` passes Apple schema validation.
4. **Gate 4**: `Info.plist` covers all 16 supported archive formats and extensions.
5. **Gate 5**: All 6 stages of `run_local_ci_gate.sh` continue to pass 100%.
