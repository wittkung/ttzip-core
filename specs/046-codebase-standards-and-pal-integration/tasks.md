# Tasks: 046-codebase-standards-and-pal-integration

**Feature Branch**: `046-codebase-standards-and-pal-integration`  
**Input Documents**: [spec.md](./spec.md), [plan.md](./plan.md), [data-model.md](./data-model.md), [research.md](./research.md), [quickstart.md](./quickstart.md), [contracts/](./contracts/)  

---

## Phase 1: Setup & Contracts Verification

- [x] T001 [P] 验证并核对 JSON Schema 契约在 `specs/046-codebase-standards-and-pal-integration/contracts/` 下的零通配与字段一致性

---

## Phase 2: User Story 1 - 安全扫描器全面接入 PAL 路径规范化 (Priority: P1)

*Goal*: 将 `SecurityScanner.swift` 全面重构为基于 `PlatformPathSanitizer`，统一 Zip Slip、DOS 保留名与 ADS 冒号流防御。  
*Independent Test*: 运行 `swift test --filter SecurityAndComplianceTests,ZipSlipDefenseTests` 全部通过。

- [x] T002 [P] [US1] 重构 `Sources/TTZipCore/SecurityScanner.swift`，接入 `PlatformPathSanitizer`

---

## Phase 3: User Story 2 - 密码存储与敏感内存防 DSE 物理擦除 (Priority: P1)

*Goal*: 将 `PasswordVaultManager.swift` 与 `ArchivePasswordStore.swift` 中的 `memset_s` 统一重构为 `PlatformMemory.secureZero`。  
*Independent Test*: 运行 `swift test --filter PasswordVaultV4Tests,ArchivePassphraseFallbackTests` 全部通过。

- [x] T003 [P] [US2] 重构 `Sources/TTZipCore/PasswordVaultManager.swift` 与 `Sources/TTZipCore/Services/ArchivePasswordStore.swift`，接入 `PlatformMemory.secureZero`

---

## Phase 4: User Story 3 - 硬件调度器与跨平台对称回退 (Priority: P2)

*Goal*: 重构 `AppleSiliconTuner.swift`，基于 `PlatformOperatingSystem` 与 `PlatformHardware` 提供平台自适应硬件拓扑。  
*Independent Test*: 运行 `swift test --filter AppleSiliconTunerTests` 全部通过。

- [x] T004 [P] [US3] 重构 `Sources/TTZipCore/AppleSiliconTuner.swift`，接入 `PlatformHardware` 与 `PlatformOperatingSystem`

---

## Phase 5: Verification & Polish

- [x] T005 运行全量 584+ 单元测试与本地 CI 流水线 `./scripts/run_local_ci.sh --quick` 验证零回归

