# Tasks: 047-libarchive-elegance-and-decoupling

**Feature Branch**: `047-libarchive-elegance-and-decoupling`  
**Input Documents**: [spec.md](./spec.md), [plan.md](./plan.md), [data-model.md](./data-model.md), [research.md](./research.md), [quickstart.md](./quickstart.md), [contracts/](./contracts/)  

---

## Phase 1: Setup & Contracts Verification

- [x] T001 [P] 验证并核对 JSON Schema 契约在 `specs/047-libarchive-elegance-and-decoupling/contracts/` 下的零通配与字段一致性

---

## Phase 2: User Story 1 - C 桥接层与 PAL 模块世界级 DocC/Doxygen 自解释注释规范化 (Priority: P1)

*Goal*: 对 `ttzip_platform.h`, `CTTZipBridge.h` 及 `Platform*.swift` 落地四维契约注释（`@brief`, `@note [Ownership]`, `@param [in]/[out]`, `@return`）。  
*Independent Test*: 编译 0 警告并通过 SwiftLint 静态质量门禁。

- [x] T002 [P] [US1] 重构 `Sources/CTTZipBridge/include/ttzip_platform.h` 与 `Sources/CTTZipBridge/include/CTTZipBridge.h`，落地四维契约注释
- [x] T003 [P] [US1] 重构 `Sources/TTZipCore/Platform/` 下的 `PlatformMemory.swift`, `PlatformPathSanitizer.swift`, `PlatformFileSystem.swift`, `PlatformHardware.swift`，落地专业级 DocC 注释与复杂度契约

---

## Phase 3: User Story 2 - 容器格式与流式滤镜正交解耦管道构建 (Priority: P1)

*Goal*: 实现 `ArchiveContainerFormat`、`ArchiveStreamFilter` 与 `ArchivePipelineCompositor`，拆解笛卡尔积复合格式，建立单向解耦管道并保留 Fast-Path 旁路。  
*Independent Test*: 运行 `swift test --filter ArchiveOrthogonalPipelineTests` 全部通过。

- [x] T004 [P] [US2] 创建 `Sources/TTZipCore/Pipeline/ArchiveContainerFormat.swift` 与 `Sources/TTZipCore/Pipeline/ArchivePipelineCompositor.swift`
- [x] T005 [P] [US2] 创建 `Tests/TTZipTests/ArchiveOrthogonalPipelineTests.swift` 验证正交组合与 Fast-Path 映射

---

## Phase 4: User Story 3 - 6 级错误码体系与状态机容错恢复模型 (Priority: P2)

*Goal*: 引入 `TTZipStatus` 与 `TTZipEngineState`，支持条目损坏时的 `dataRecovery` 优雅跳过与继续解压。  
*Independent Test*: 运行 `swift test --filter TTZipStatusAndRecoveryTests` 全部通过。

- [x] T006 [P] [US3] 创建 `Sources/TTZipCore/Pipeline/TTZipStatus.swift`
- [x] T007 [P] [US3] 创建 `Tests/TTZipTests/TTZipStatusAndRecoveryTests.swift`

---

## Phase 5: Verification & Polish

- [x] T008 运行全量 584+ 单元测试与本地 CI 流水线 `./scripts/run_local_ci.sh --quick` 验证零回归

