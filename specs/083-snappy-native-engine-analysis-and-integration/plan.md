# Implementation Plan: Google Snappy 原生引擎深度剖析与架构集成 (083-snappy-native-engine-analysis-and-integration)

**Feature Branch**: `083-snappy-native-engine-analysis-and-integration`  
**Created**: 2026-08-18  
**Status**: Ready for Tasks  
**Feature Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/083-snappy-native-engine-analysis-and-integration/spec.md)

---

## 1. Technical Context

- **语言与标准**：Swift 6.0 (`swift-tools-version: 6.0`)、C11/POSIX、C++17 (Google Snappy 核心引擎)。
- **目标平台**：macOS 14.0+ (Sonoma / Sequoia)，Apple Silicon (ARM64) 优先，兼容 Intel (x86_64)。
- **底层 C 桥接中枢**：`Sources/CTTZipBridge/`，静态链接系统 `libc++` 与 `libarchive`，零外部 CLI 进程派生。
- **硬件加速**：Apple Silicon ARM64 ACLE Castagnoli CRC32C 硬件指令（`__builtin_arm_crc32cd` 4 路展开）+ Slice-by-8 软件降级。

---

## 2. Constitution Check

- [x] **热路径零成本抽象 (Zero-Cost Abstraction)**：编解码循环内零中间堆分配、零锁竞争、零动态对象树，哈希表（32KB）驻留 L1 Cache。
- [x] **Fast-Path 旁路保留**：原生 Snappy 块与 Framing 帧直通 C 原语，不经由通用慢路径。
- [x] **吞吐硬门禁对齐**：Snappy 内存解压吞吐底线保持 $\ge 4,500\text{ MB/s}$（Debug）/ $\ge 6,000\text{ MB/s}$（Release）。
- [x] **MAS 沙盒与跨平台合规**：彻底剔除 `archive_write_add_filter_program(a, "snappy")`，100% 进程内内存管道。
- [x] **SPDX 版权与规范化**：所有新增与修改文件顶部包含标准 SPDX Header。

---

## 3. Phase 0: Research Index

- - R001 [SUBAGENT:research] 《Google Snappy 源码架构与 C 桥接中枢接入方案》：详见 [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/083-snappy-native-engine-analysis-and-integration/research.md#1-r001-google-snappy-源码架构与-c-桥接中枢设计)
- - R002 [SUBAGENT:research] 《Snappy 官方 Framing Format 规范与 Apple Silicon ARM64 CRC32C 硬件加速》：详见 [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/083-snappy-native-engine-analysis-and-integration/research.md#2-r002-snappy-官方-framing-format-规范与-apple-silicon-arm64-crc32c-硬件加速)
- - R003 [SUBAGENT:research] 《100% 进程内 TAR.SZ 流式管道与 Libarchive 自定义回调》：详见 [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/083-snappy-native-engine-analysis-and-integration/research.md#3-r003-100-进程内-tarsz-流式管道与-libarchive-自定义回调)
- - R004 [SUBAGENT:research] 《不可信输入/恶意损坏流内存安全防御与 13 维 Fuzzing 矩阵》：详见 [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/083-snappy-native-engine-analysis-and-integration/research.md#4-r004-不可信输入恶意损坏流内存安全防御与-13-维-fuzzing-矩阵)

---

## 4. Phase 1: Architecture & Design Artifacts

- **数据模型**：[data-model.md](file:///Users/kevintung/Documents/dev/TTZip/specs/083-snappy-native-engine-analysis-and-integration/data-model.md)
- **接口契约**：
  - [contracts/snappy_block_contract.json](file:///Users/kevintung/Documents/dev/TTZip/specs/083-snappy-native-engine-analysis-and-integration/contracts/snappy_block_contract.json) `[SUBAGENT:research]`
  - [contracts/snappy_framing_contract.json](file:///Users/kevintung/Documents/dev/TTZip/specs/083-snappy-native-engine-analysis-and-integration/contracts/snappy_framing_contract.json) `[SUBAGENT:research]`
  - [contracts/snappy_tar_pipeline_contract.json](file:///Users/kevintung/Documents/dev/TTZip/specs/083-snappy-native-engine-analysis-and-integration/contracts/snappy_tar_pipeline_contract.json) `[SUBAGENT:research]`
- **验证指南**：[quickstart.md](file:///Users/kevintung/Documents/dev/TTZip/specs/083-snappy-native-engine-analysis-and-integration/quickstart.md)

---

## 5. Component Changes Breakdown

### 5.1 C 底层引擎与桥接层 (`Sources/CTTZipBridge/`)
- [NEW] `Sources/CTTZipBridge/snappy/`: 嵌入 Google Snappy 原生核心源码（`snappy.h`, `snappy-c.h`, `snappy.cc`, `snappy-c.cc`, `snappy-internal.h`, `snappy-stubs-internal.h`, `snappy-stubs-public.h`）。
- [NEW] `Sources/CTTZipBridge/include/CTTZipBridge_Snappy.h`: 声明纯 C11 块编解码与 Framing 流式帧 API。
- [NEW] `Sources/CTTZipBridge/CTTZipBridge_Snappy.c`: 实现 C 桥接、CRC32C 硬件加速与流式帧编解码状态机。
- [MODIFY] `Sources/CTTZipBridge/include/CTTZipBridge.h`: 导出 Snappy 桥接符号与 `ttzip_create_tar_snappy_native_c` / `ttzip_extract_tar_snappy_native_c`。
- [MODIFY] `Sources/CTTZipBridge/ttzip_tar_native.c`: 接入进程内 Snappy 回调，替换掉外部子进程 `archive_write_add_filter_program(a, "snappy")`。
- [MODIFY] `Sources/CTTZipBridge/CTTZipBridge_Archive.c`: 路由 `snappy` 格式到原生进程内引擎。

### 5.2 Swift 核心引擎层 (`Sources/TTZipCore/`)
- [NEW] `Sources/TTZipCore/Snappy/SnappyBlockEngine.swift`: 提供 Swift 内存块原生编解码器与强类型错误封装。
- [NEW] `Sources/TTZipCore/Snappy/SnappyFramingStream.swift`: 提供遵循 Framing Format 规范的 Swift 流式帧封装。
- [NEW] `Sources/TTZipCore/Snappy/SnappyError.swift`: 强类型 Snappy 错误枚举与本地化描述。
- [MODIFY] `Sources/TTZipCore/ArchiveWriter+Dispatch.swift`: 更新 Snappy 写入分发逻辑。
- [MODIFY] `Sources/TTZipCore/TemplateMethod/TarArchiveEngineTemplate.swift`: 完善 Snappy 模板方法。

### 5.3 测试与基准套件 (`Tests/TTZipTests/`)
- [NEW] `Tests/TTZipTests/SnappyBlockEngineTests.swift`: 块编解码正确性、极大/极小数据与一致性单测。
- [NEW] `Tests/TTZipTests/SnappyFramingStreamTests.swift`: Framing 帧格式解析、Stream ID 匹配与 CRC32C 校验单测。
- [NEW] `Tests/TTZipTests/SnappySecurityAndFuzzingTests.swift`: 13 维逆向变异、畸形包注入与内存安全模糊测试。
- [NEW] `Tests/TTZipTests/TarSnappyInProcessTests.swift`: 100% 进程内 TAR.SZ 归档打包与解压端到端测试。
- [MODIFY] `Tests/TTZipTests/AllFormatsAndAdvancedParametersMatrixTests.swift`: 解除 `testFormat_SNAPPY()` 的 `throw XCTSkip` 限制，激活原生断言。
