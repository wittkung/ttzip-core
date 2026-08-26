# Implementation Plan: 075 Mac App Store Release Sprint

## 1. Technical Context

TTZip is ready for MAS submission. The release packaging pipeline must automate:
1. ICNS generation from Retina master icon.
2. Construction of `PrivacyInfo.xcprivacy` conforming to Apple's XML plist schema.
3. Hardening `Info.plist` with all 16 format UTIs and `TTZip.entitlements` with required App Sandbox keys.
4. Implementing `scripts/package_mas_app.sh` that builds `-DMAS_BUILD -c release`, generates `.app` bundle, copies resources, signs with sandbox entitlements, and runs verification.

## 2. Component Changes

- `Sources/TTZipApp/PrivacyInfo.xcprivacy` [NEW]
- `Sources/TTZipApp/TTZip.entitlements` [MODIFY]
- `Sources/TTZipApp/Info.plist` [MODIFY]
- `Sources/TTZipApp/Resources/` [NEW]
- `scripts/generate_app_icon.sh` [NEW]
- `scripts/package_mas_app.sh` [NEW]
- `Tests/TTZipTests/AppStorePackageAuditTests.swift` [NEW]
