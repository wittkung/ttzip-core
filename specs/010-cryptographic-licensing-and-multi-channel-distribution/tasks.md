# Tasks: TTZip 密码学离线授权、Steam 商店上架与四轨分发体系 (Feature 010)

- **Feature ID**: `010-cryptographic-licensing-and-multi-channel-distribution`
- **Specification**: [`specs/010-cryptographic-licensing-and-multi-channel-distribution/spec.md`](file:///Users/kevintung/Documents/dev/products/ttzip/specs/010-cryptographic-licensing-and-multi-channel-distribution/spec.md)
- **Implementation Plan**: [`specs/010-cryptographic-licensing-and-multi-channel-distribution/plan.md`](file:///Users/kevintung/Documents/dev/products/ttzip/specs/010-cryptographic-licensing-and-multi-channel-distribution/plan.md)
- **Status**: `COMPLETED`

---

## Phase 1: Setup & Licensing Infrastructure (T001 - T002)

- [x] T001 [P] Create Python license key generator in `core/scripts/generate_license.py` to generate Ed25519 keypairs, sign payloads, and emit `TTZIP1-<Payload>.<Sig>` tokens
- [x] T002 [P] Create schema definition and test vectors for offline license payloads in `core/Tests/TTZipTests/Fixtures/license_test_vectors.json`

---

## Phase 2: Foundational CryptoKit Verification Engine (T003 - T004)

- [x] T003 Implement `Ed25519LicenseVerifier` using Apple `CryptoKit` in `core/Sources/TTZipCore/Security/Ed25519LicenseVerifier.swift`
- [x] T004 Add unit test suite `Ed25519LicenseVerifierTests` in `core/Tests/TTZipTests/Ed25519LicenseVerifierTests.swift` validating valid keys, forged payloads, corrupted signatures, and malformed strings

---

## Phase 3: User Story 1 - 离线密码学授权管理与状态持久化 (T005 - T008)

- [x] T005 [US1] Define `LicensePayload` and `ChannelLicenseState` in `core/Sources/TTZipCore/Security/LicenseModels.swift`
- [x] T006 [US1] Implement `Ed25519LicenseManager` actor/class in `core/Sources/TTZipCore/Security/Ed25519LicenseManager.swift` with UserDefaults/Keychain storage and channel detection
- [x] T007 [P] [US1] Update `apple/Sources/TTZipApp/ViewModels/SettingsViewModel.swift` (or `SettingsView.swift`) to bind with `Ed25519LicenseManager`
- [x] T008 [US1] Update `apple/Sources/TTZipApp/Views/SettingsView+Tabs.swift` to render channel badges (Community, Direct Pro, MAS Pro, Steam Pro) and license activation/deactivation controls

---

## Phase 4: User Story 2 - 清理硬编码后门与废除伪门禁 (T009 - T011)

- [x] T009 [US2] Remove `_ = activate(key: "AURA-PRO1-KEY8-2026")` and fake `validateKeyFormat` string matching from `core/Sources/TTZipCore/SystemServices.swift`
- [x] T010 [US2] Remove artificial `isPro` check on Ultra compression in `core/Sources/TTZipCore/ArchiveWriter.swift` ensuring 100% full feature availability in Community builds
- [x] T011 [US2] Verify `swift test --package-path core` and `swift test --package-path apple` pass with clean license architecture

---

## Phase 5: User Story 3 - Mac App Store 与 Steam 商店免激活合规 (T012 - T014)

- [x] T012 [P] [US3] Configure compilation flags `-DMAS_BUILD` and `-DSTEAM_BUILD` in `apple/Package.swift` and `core/Package.swift`
- [x] T013 [US3] Ensure `Ed25519LicenseManager` automatically reports `ChannelLicenseState.masPro` under `-DMAS_BUILD` and `ChannelLicenseState.steamPro` under `-DSTEAM_BUILD`
- [x] T014 [US3] Add unit test suite `ChannelDistributionTests` in `apple/Tests/TTZipAppTests/ChannelDistributionTests.swift` validating channel badge resolution under all builds

---

## Phase 6: User Story 4 - 多渠道参数化打包与开源治理 (T015 - T018)

- [x] T015 [P] [US4] Update `apple/scripts/bundle_app.sh` to accept `--channel [direct|mas|steam|community]` and apply corresponding compilation flags, entitlements, and Sparkle framework inclusion
- [x] T016 [P] [US4] Update `core/scripts/package_local_release.sh` to support `--channel` parameterization
- [x] T017 [P] [US4] Add `apple/CONTRIBUTING.md` and `apple/SECURITY.md` aligning governance policies with `core/`
- [x] T018 [US4] Run full verification: `swift test`, LOC gate defense ($\le 800$ LOC), license audit, and multi-channel bundle test
