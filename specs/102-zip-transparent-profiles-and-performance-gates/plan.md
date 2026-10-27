# Implementation Plan: ZIP 强类型透明 Profile 架构与全量性能门禁重构

## 1. Technical Context
- **语言与平台**：Swift 6.0 + C11，macOS 14+ (Sonoma)，Apple Silicon M 系列优先。
- **模块架构**：`TTZipCore` (Zip/ZipExtremeBlockWriter.swift, ArchiveCompressionTypes.swift) -> `CTTZipBridge` (ttzip_zopfli_engine.c) -> `TTZipTests` (XCTestPerformanceMeasureTests.swift, ZipMultiCoreParetoFrontierPkTests.swift)。
- **核心目标**：
  1. 引入强类型 `ZipCompressionProfile`，彻底移除隐式 `effectiveZipRawLevel` 脏映射；
  2. 统一底层 C 桥接结构体 `TTZipZopfliOptions` 与 Profile 的 1:1 参数透传；
  3. 重构并加固 `XCTestPerformanceMeasureTests.swift` 与 `ZipMultiCoreParetoFrontierPkTests.swift` 的 8 大黄金档位性能门禁断言。

## 2. Constitution & Rules Check
- [x] **性能铁律**：热路径零堆分配、零锁并发、保持大块无锁并行。
- [x] **架构设计**：采用强类型 Value Object / Strategy 模式，消除散落黑盒 switch。
- [x] **零幻觉与物理可验证**：所有门禁基于 18 核心真实物理测试，0 failures 0 warnings。

## 3. Phase 0: Research Items
- - R001 [SUBAGENT:research] 《强类型 `ZipCompressionProfile` 结构体参数与 libdeflate / in-process Zopfli C 引擎桥接映射研究》：已完成，见 [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/102-zip-transparent-profiles-and-performance-gates/research.md)。
- - R002 [SUBAGENT:research] 《Apple Silicon M 系列 18 核心在 8 大黄金档位下的物理吞吐硬门禁标定研究》：已完成，见 [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/102-zip-transparent-profiles-and-performance-gates/research.md)。

## 4. Phase 1: Design Artifacts
- **Data Model**: [data-model.md](file:///Users/kevintung/Documents/dev/TTZip/specs/102-zip-transparent-profiles-and-performance-gates/data-model.md)
- **Contracts**: [contracts/zip-compression-profile.schema.json](file:///Users/kevintung/Documents/dev/TTZip/specs/102-zip-transparent-profiles-and-performance-gates/contracts/zip-compression-profile.schema.json)
- **Quickstart**: [quickstart.md](file:///Users/kevintung/Documents/dev/TTZip/specs/102-zip-transparent-profiles-and-performance-gates/quickstart.md)

## 5. Component Modification List
1. **[NEW] `Sources/TTZipCore/Zip/ZipCompressionProfile.swift`**:
   - 定义 `ZipCompressionProfile` 强类型结构体及 8 大标准静态 profile。
2. **[MODIFY] `Sources/TTZipCore/ArchiveCompressionTypes.swift`**:
   - 移除 `effectiveZipRawLevel` 脏属性，桥接至 `ZipCompressionProfile.profile(for: level)`。
3. **[MODIFY] `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift`**:
   - 直接接收 `ZipCompressionProfile` 或透明解析 profile 传给 C 结构体 `TTZipZopfliOptions`。
4. **[MODIFY] `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift`**:
   - 全量对齐最新 Profile 档位与 8 大黄金门禁。
5. **[MODIFY] `Tests/TTZipTests/ZipMultiCoreParetoFrontierPkTests.swift`**:
   - 使用强类型 Profile 遍历 8 大黄金档位，确保图表与门禁完全吻合。
