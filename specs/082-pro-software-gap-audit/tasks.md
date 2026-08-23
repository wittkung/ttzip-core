# Tasks: TTZip 全维度专业归档能力补齐与工程落地

**Input**: Design documents from `/specs/082-pro-software-gap-audit/`  
**Prerequisites**: [plan.md](file:///Users/kevintung/Documents/dev/TTZip/specs/082-pro-software-gap-audit/plan.md), [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/082-pro-software-gap-audit/spec.md), [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/082-pro-software-gap-audit/research.md), [data-model.md](file:///Users/kevintung/Documents/dev/TTZip/specs/082-pro-software-gap-audit/data-model.md), [contracts/](file:///Users/kevintung/Documents/dev/TTZip/specs/082-pro-software-gap-audit/contracts/)

---

## Format: `[ID] [P?] [Story] Description`
- **[P]**: 并行任务（无文件与依赖冲突）
- **[Story]**: 所属用户故事（US1 ~ US6）
- 必须标明具体目标文件绝对/相对路径

---

## Phase 1: Shared Infrastructure & Foundational

- [x] T001 [P] [Foundation] 校验并对齐 `ArchiveFilterOptions.swift` 中的系统元数据谓词（`__MACOSX`, `._*`, `.DS_Store`, `Thumbs.db`）在 `Sources/TTZipCore/ArchiveFilterOptions.swift`
- [x] T002 [P] [Foundation] 扩展 `TouchIDAuthenticator.swift` 接入 `LocalAuthentication` 框架 `LAPolicy.deviceOwnerAuthentication` 在 `Sources/TTZipCore/Security/TouchIDAuthenticator.swift`
- [x] T003 [P] [Foundation] 扩展 `TempDirectoryCleanUpManager.swift` 增加 `TTZipEdit_*` 隔离沙盒自动回收中枢在 `Sources/TTZipCore/Utilities/TempDirectoryCleanUpManager.swift`

---

## Phase 2: User Story 1 - 智能解压与操作后自动化流水线 (Priority: P1) 🎯 MVP

**Goal**: 依据包内有效根节点数量自动判断是否生成同名包裹文件夹，杜绝双层嵌套与桌面散落，并在解压后执行废纸篓移动与 Finder 高亮。

- [x] T004 [P] [US1] 编写智能解压与路径决策单元测试用例在 `Tests/TTZipTests/SmartExtractionTests.swift`
- [x] T005 [P] [US1] 实现 `SmartExtractResolver` 核心两阶段有效根求解算法在 `Sources/TTZipCore/Security/PathPatternFilterEngine.swift`
- [x] T006 [US1] 在 `ArchiveExtractor+Dispatch.swift` 中接入 `SmartExtractResolver` 决策管道在 `Sources/TTZipCore/ArchiveExtractor+Dispatch.swift`
- [x] T007 [US1] 在 `AppViewState.swift` 中接入智能解压、操作后自动移入废纸篓与 Finder 定位联动在 `Sources/TTZipApp/ViewModels/AppViewState.swift`

---

## Phase 3: User Story 2 - 多格式自适应分卷归档切片与合并 (Priority: P1)

**Goal**: 零拷贝流式跨卷切片管道，支持 7Z (`.7z.001`) 与 ZIP (`.z01`/`.zip.001`) 自适应切片创建与连续卷探测合并。

- [x] T008 [P] [US2] 编写多卷分卷切片与合并差分测试在 `Tests/TTZipTests/SplitVolumeSpanningTests.swift`
- [x] T009 [P] [US2] 在 C 桥接层强化 7Z 首卷 32 字节延迟回写修补逻辑在 `Sources/CTTZipBridge/ttzip_7z_header_writer.c`
- [x] T010 [US2] 强化 `MultiVolumeStreamSink.swift` 零拷贝边界切换与原子重命名在 `Sources/TTZipCore/Split/MultiVolumeStreamSink.swift`
- [x] T011 [US2] 在 `CompressModalView.swift` 中暴露分卷大小预设选择器（CD, DVD, FAT32, 25MB, 100MB, 自定义）在 `Sources/TTZipApp/Views/CompressModalView.swift`

---

## Phase 4: User Story 3 - 7Z 头部文件名加密与 Touch ID 生物识别 (Priority: P1)

**Goal**: 7Z AES-256 Central Directory 头部加密 (`-mhe`) 与 macOS Touch ID / Apple Watch 生物认证安全集成。

- [x] T012 [P] [US3] 编写 7Z EncodedHeader 加密解析与 Touch ID 认证测试在 `Tests/TTZipTests/HeaderEncryptionTests.swift`
- [x] T013 [P] [US3] 完善 `ttzip_7z_header_parser.c` 中 `kEncodedHeader` (ID 0x17) NEON KDF 派生状态机在 `Sources/CTTZipBridge/ttzip_7z_header_parser.c`
- [x] T014 [US3] 在 `PasswordVaultManager+Keychain.swift` 中绑定 `SecAccessControl` 与 `LAContext` 硬件级授权在 `Sources/TTZipCore/PasswordVaultManager+Keychain.swift`
- [x] T015 [US3] 在 `PasswordPromptSheetView.swift` 中集成 Touch ID 一键快捷解锁在 `Sources/TTZipApp/Views/PasswordPromptSheetView.swift`

---

## Phase 5: User Story 4 - 外部应用程序就地编辑与双向热回写 (Priority: P2)

**Goal**: 双击单条目提取到 UUID 沙盒，拉起外部编辑器，通过 Dual-Tier `DispatchSource` 侦测保存并原子重压缩回写。

- [x] T016 [P] [US4] 编写外部编辑会话与 Inode 替换文件监听测试在 `Tests/TTZipTests/InPlaceEditSessionTests.swift`
- [x] T017 [P] [US4] 强化 `FileWatcherEngine.swift` 的 Dual-Tier（父目录+文件）FD 协同监听在 `Sources/TTZipCore/FileWatcherEngine.swift`
- [x] T018 [US4] 在 `InPlaceArchiveMutationEngine.swift` 中强化基于 Actor 的串行写锁与 APFS 影子替换在 `Sources/TTZipCore/InPlaceEdit/InPlaceArchiveMutationEngine.swift`
- [x] T019 [US4] 在 `ArchiveExplorerView.swift` 中打通外部编辑双击拉起与保存提示交互在 `Sources/TTZipApp/Views/ArchiveExplorerView.swift`

---

## Phase 6: User Story 5 - 灾难自愈 Reed-Solomon 恢复记录与前向纠错 (Priority: P2)

**Goal**: 基于 $GF(2^{16})$ Cauchy Reed-Solomon 算法在归档尾部附加 1% ~ 10% 恢复记录，损坏时自动定位坏块并重建数据。

- [x] T020 [P] [US5] 编写 Reed-Solomon 纠错与坏块注入自愈测试在 `Tests/TTZipTests/ReedSolomonRecoveryRecordTests.swift`
- [x] T021 [P] [US5] 实现 ARM NEON `PMULL` 加速的 $GF(2^{16})$ Cauchy RS 编解码内核在 `Sources/CTTZipBridge/ttzip_rs_fec.c`
- [x] T022 [US5] 扩展 `ArchiveRepairEngine.swift` 实现尾部 `TTZIP_RR\x01` 解析与坏块切片求解在 `Sources/TTZipCore/ArchiveRepairEngine.swift`
- [x] T023 [US5] 在 `ArchiveIntegrityView.swift` 中展示恢复记录检测状态与一键自愈修复在 `Sources/TTZipApp/Views/ArchiveIntegrityView.swift`

---

## Phase 7: User Story 6 - GUI 原生多核算力能效基准仪表盘 (Priority: P3)

**Goal**: 在 SwiftUI 界面中提供美观实时的 Benchmark 仪表盘（对标 7-Zip MIPS），展示多核吞吐与能效曲线。

- [x] T024 [P] [US6] 编写 GUI Benchmark 数据发布器与 MIPS 计算模型测试在 `Tests/TTZipTests/MIPSBenchmarkEngineTests.swift`
- [x] T025 [US6] 在 `BenchmarkView.swift` 中绘制实时压缩/解压吞吐折线图与 MIPS 评分仪表盘在 `Sources/TTZipApp/Views/BenchmarkView.swift`

---

## Phase 8: Verification, Quality Gate & Performance Floor

- [x] T026 全量回归单元测试套件 (`swift test`) 验证 525+ 测试用例 100% 通过
- [x] T027 性能门禁与热路径吞吐测试 (`swift test --filter XCTestPerformanceMeasureTests`) 验证零倒退
- [x] T028 执行 `speckit-analyze` 跨工件一致性扫描与 Schema 严谨性闭环

