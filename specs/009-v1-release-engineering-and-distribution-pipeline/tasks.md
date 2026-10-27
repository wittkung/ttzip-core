# Tasks: TTZip v1.0.0 生产级发布工程与全生态自动化分发流水线 (Feature 009)

- **Feature ID**: `009-v1-release-engineering-and-distribution-pipeline`
- **Specification**: [`specs/009-v1-release-engineering-and-distribution-pipeline/spec.md`](file:///Users/kevintung/Documents/dev/products/ttzip/specs/009-v1-release-engineering-and-distribution-pipeline/spec.md)
- **Implementation Plan**: [`specs/009-v1-release-engineering-and-distribution-pipeline/plan.md`](file:///Users/kevintung/Documents/dev/products/ttzip/specs/009-v1-release-engineering-and-distribution-pipeline/plan.md)
- **Status**: `COMPLETED`

---

## Phase 1: SPM 目标拓扑解耦与双端测试闭环 (T001 - T004)

- [x] T001 [P] [US1] Clean up `core/Package.swift` to remove redundant `TTZipApp`, `TTZipFinderSync`, `TTZipQuickLook`, `TTZipAppTests` targets, leaving only core SDK targets
- [x] T002 [P] [US1] Update `apple/Package.swift` to reference local `../core` and ensure clean target dependency binding
- [x] T003 [US1] Execute `swift test --package-path core` and verify all 166 Swift core test cases pass
- [x] T004 [US1] Execute `swift test --package-path apple` and verify all 17 Apple client test suites pass

---

## Phase 2: 许可证合规、SPDX 头部注入与 LOC 防御门禁 (T005 - T007)

- [x] T005 [P] [US2] Run `inject_spdx_headers.py` and ensure generated UniFFI files have valid SPDX license headers
- [x] T006 [US2] Run `python3 core/scripts/audit_licenses.py` and verify 100% license & IP compliance pass
- [x] T007 [US2] Run LOC gate checks on `core` and `apple` verifying zero files exceed 800 LOC threshold

---

## Phase 3: macOS 客户端 Release 封装、Retina DMG 制作与 Sparkle 2.0 (T008 - T011)

- [x] T008 [P] [US3] Execute `apple/scripts/bundle_app.sh` to assemble and ad-hoc sign `dist/TTZip.app`
- [x] T009 [P] [US3] Execute `apple/scripts/create_dmg_installer.sh` to generate Retina UDZO `dist/TTZip-1.0.0.dmg`
- [x] T010 [US3] Execute `apple/scripts/notarize_dmg.sh --diagnose` to perform Gatekeeper assessment
- [x] T011 [P] [US3] Execute `apple/scripts/generate_appcast.sh` to generate `apple/appcast.xml`

---

## Phase 4: CLI 发布包组装、SHA256 Manifest 与 Homebrew 对齐 (T012 - T015)

- [x] T012 [P] [US4] Build Release binary `ttzip` CLI via `cargo build --release -p ttzip-tui`
- [x] T013 [P] [US4] Package standalone `dist/ttzip-cli-v1.0.0-darwin-universal.tar.gz` with man page and completions
- [x] T014 [P] [US4] Generate `dist/checksums.txt` containing SHA256 hashes for CLI tarball and DMG installer
- [x] T015 [P] [US4] Validate `homebrew/Formula/ttzip.rb` syntax, test block, and consistency

---

## Phase 5: 全域质检验收与变更落地 (T016 - T018)

- [x] T016 [US1] Run `make -C core test-all-sdk` verifying all SDK test suites pass
- [x] T017 [US1] Run `make -C core test-out-of-tree-smoke` verifying clean container out-of-tree smoke test pass
- [x] T018 Final git status audit and release readiness sign-off
