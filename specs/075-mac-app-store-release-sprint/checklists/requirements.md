# Requirements Checklist: 075 Mac App Store Release Sprint

## 1. Content Quality & App Store Compliance
- [x] Hard sandbox enabled (`com.apple.security.app-sandbox: true`)
- [x] Zero network access in MAS build (`#if !MAS_BUILD` on Sparkle / updates)
- [x] Security-scoped bookmarks enabled for persistent folder access
- [x] Privacy Manifest (`PrivacyInfo.xcprivacy`) properly configured
- [x] Retina AppIcon ICNS assets created

## 2. Requirement Completeness
- [x] 16 format UTI definitions in `Info.plist`
- [x] Document Type bindings for Editor role
- [x] Bilingual `InfoPlist.strings` (English & Simplified Chinese)
- [x] Automated packaging script (`scripts/package_mas_app.sh`)

## 3. Feature Readiness
- [x] Standalone executable and `.app` bundle verification
- [x] CI regression gates verification
