# Tasks: Google Snappy 原生引擎深度剖析与架构集成 (083-snappy-native-engine-analysis-and-integration)

**Feature Branch**: `083-snappy-native-engine-analysis-and-integration`  
**Created**: 2026-08-18  
**Status**: Ready for Implementation  
**Feature Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/083-snappy-native-engine-analysis-and-integration/spec.md)  
**Implementation Plan**: [plan.md](file:///Users/kevintung/Documents/dev/TTZip/specs/083-snappy-native-engine-analysis-and-integration/plan.md)

---

## Phase 1: Setup & C 底层引擎与桥接中枢 (Foundational)

**Purpose**: 建立 100% 进程内 Google Snappy 静态核心源码与 C11 桥接中枢，彻底消除外部子进程依赖。

- [x] T001 [P] [US2] 静态嵌入 Google Snappy 原生 C/C++ 核心源文件在 `Sources/CTTZipBridge/snappy/`
- [x] T002 [P] [US2] 创建纯 C11 桥接接口头文件在 `Sources/CTTZipBridge/include/CTTZipBridge_Snappy.h`
- [x] T003 [US2] 实现 C 桥接引擎、ARM64 ACLE CRC32C 硬件加速与分块帧编解码器在 `Sources/CTTZipBridge/CTTZipBridge_Snappy.c`
- [x] T004 [US2] 导出 Snappy 桥接符号与 TAR 管道函数在 `Sources/CTTZipBridge/include/CTTZipBridge.h`

---

## Phase 2: User Story 2 - Raw Block 编解码与 Swift 核心引擎 (Priority: P1) 🎯 MVP

**Goal**: 实现内存块到内存块的高吞吐 Snappy 编解码与强类型错误封装。

- [x] T005 [P] [US2] 实现强类型错误枚举模型在 `Sources/TTZipCore/Snappy/SnappyError.swift`
- [x] T006 [P] [US2] 实现原生 Snappy 块编解码引擎在 `Sources/TTZipCore/Snappy/SnappyBlockEngine.swift`
- [x] T007 [P] [US2] 创建 Snappy 块编解码单元测试集在 `Tests/TTZipTests/SnappyBlockEngineTests.swift`

---

## Phase 3: User Story 3 - Framing Format 流式帧与 TAR.SZ 进程内管道 (Priority: P1)

**Goal**: 遵循官方 Framing 规范构建流式分块帧引擎，打通 TAR.SZ 零外部 CLI 进程内归档与解压闭环。

- [x] T008 [P] [US3] 实现 Snappy Framing 帧流编解码在 `Sources/TTZipCore/Snappy/SnappyFramingStream.swift`
- [x] T009 [P] [US3] 创建 Snappy Framing 流式单元测试集在 `Tests/TTZipTests/SnappyFramingStreamTests.swift`
- [x] T010 [US3] 桥接 100% 进程内 TAR.SZ 归档与解压回调在 `Sources/CTTZipBridge/ttzip_tar_native.c` 与 `Sources/CTTZipBridge/CTTZipBridge_Archive.c`
- [x] T011 [US3] 接入 ArchiveWriter 与 TarArchiveEngineTemplate 在 `Sources/TTZipCore/ArchiveWriter+Dispatch.swift` 与 `Sources/TTZipCore/TemplateMethod/TarArchiveEngineTemplate.swift`
- [x] T012 [P] [US3] 创建 TAR.SZ 进程内集成测试在 `Tests/TTZipTests/TarSnappyInProcessTests.swift`

---

## Phase 4: User Story 4 - 不可信输入与损坏流内存安全防御 (Priority: P2)

**Goal**: 通过 13 维逆向变异与畸形流注入测试，断言引擎在任意损坏流下 100% 优雅捕获、零不可信崩溃。

- [x] T013 [P] [US4] 实现 13 维逆向注入与模糊测试集在 `Tests/TTZipTests/SnappySecurityAndFuzzingTests.swift`

---

## Phase 5: User Story 5 - 全格式回归、门禁验证与跳过解除 (Priority: P2)

**Goal**: 解除历史测试跳过标记，执行全量 525+ 测试断言零倒退。

- [x] T014 [US5] 解除 `Tests/TTZipTests/AllFormatsAndAdvancedParametersMatrixTests.swift` 中 `testFormat_SNAPPY` 的跳过状态并实装完整闭环测试与性能门禁回归验证
- [x] T015 [US5] 执行全量单元测试与性能门禁回归验证
