# Feature Specification: ZIP 强类型透明 Profile 架构与全量性能门禁重构

## 1. 业务背景与问题定义 (Problem Statement)
当前 TTZip 的 ZIP 压缩引擎存在底层物理参数与顶层通用枚举（`ArchiveCompressionLevel`）之间的“转接头隐式映射（`effectiveZipRawLevel`）”技术债务：
1. **隐式重映射与契约模糊**：调用方传入 `level 5`，底层悄悄执行 `raw 10`；测试用例容易发生“二次映射”导致档位漂移和算法重叠（如之前 `L3` 与 `L5` 重合）；
2. **格式与通用层深度耦合**：通用枚举被 ZIP 特有逻辑污染，违背单一职责（SRP）与开闭原则（OCP）；
3. **Swift 模式匹配别名拦截缺陷**：便利属性（如 `static var medium = .level5`）在 switch 匹配时容易被其他分支拦截；
4. **性能门禁陈旧脱节**：`XCTestPerformanceMeasureTests.swift` 中的门禁测试仍沿用旧的通用枚举别名（如 `.normal`），门禁阈值与新 8 大黄金档位脱节。

## 2. 用户场景与用户故事 (User Scenarios & User Stories)

### User Story 1: 强类型透明 ZIP 压缩配置 Profile (Transparent Compression Profile)
- **作为** TTZip 核心引擎开发者与上层调用方，
- **我希望** 拥有强类型、零隐式转换的 `ZipCompressionProfile`（包含 `deflateLevel`, `zopfliIterations`, `blockSplitting`, `targetThroughputFloorMBs`），
- **以便于** 每一个档位的物理执行参数和性能预期 100% 透明确定，杜绝黑盒转换与隐式猜测。

### User Story 2: 7 大压缩档位 + Store 物理参数与枚举纯净对齐 (Pure Enum & Engine Alignment)
- **作为** 归档管线，
- **我希望** `ArchiveCompressionLevel` 内部只表达纯粹的抽象等级（.store, .level1 ... .level7），并通过统一的 Profile 工厂映射至各档位的最优真实算法，
- **以便于** 严格保证各个档位单调递进、零重复重叠。

### User Story 3: 现代化 8 大黄金档位性能硬门禁矩阵 (Comprehensive Performance Gates)
- **作为** CI/CD 与质量保障工程师，
- **我希望** `XCTestPerformanceMeasureTests.swift` 覆盖最新的 Profile 档位，
- **以便于** 在 Debug 和 Release 模式下严格拦截任何吞吐退化与压缩率倒退。

## 3. 功能需求清单 (Functional Requirements)
- **FR-001**: 定义强类型结构体 `ZipCompressionProfile: Sendable, Equatable, Hashable`，包含 `name`, `level`, `deflateLevel: Int32`, `zopfliIterations: Int32`, `blockSplitting: Bool`, `targetThroughputFloorMBs: Double`。
- **FR-002**: 提供 8 大标准静态 Profile：`.store`, `.fast (L1)`, `.fastPlus (L2)`, `.normal (L3)`, `.maximum (L4)`, `.graphFast (L5)`, `.ultraZopfli (L6)`, `.extremePeak (L7)`。
- **FR-003**: 移除 `ArchiveCompressionTypes.swift` 中对特定格式有副作用的 `effectiveZipRawLevel` 脏逻辑，改为由 `ZipCompressionProfile.forLevel(level)` 进行透明派发。
- **FR-004**: 重构 `ZipExtremeBlockWriter.swift` 和 `ArchiveWriter.swift`，直接消费 `ZipCompressionProfile` 进行分块并发压缩。
- **FR-005**: 重构 `XCTestPerformanceMeasureTests.swift`，根据新 Profile 矩阵建立 8 大档位的精确吞吐硬门禁。
- **FR-006**: 修复 `CompetitorBenchmarkCacheManager`，断言 TTZip 自身全量档位 100% 物理现场实测，杜绝旧数据干扰。

## 4. 成功衡量指标 (Success Criteria)
- **SC-001**: 运行 `swift test --filter XCTestPerformanceMeasureTests` 100% 通过且无任何门禁告警。
- **SC-002**: 运行 `swift test --filter ZipMultiCoreParetoFrontierPkTests`，8 大黄金档位物理体积单调严格递减，无任何重叠点。
- **SC-003**: 零隐式 switch 重映射，所有档位参数在 `ZipCompressionProfile` 中 100% 显式声明。

## 5. 澄清与会话记录 (Clarifications)
- **C-001**: 为什么不保留旧的 `effectiveZipRawLevel`？
  - 答：保留会导致心智负担和跨层污染，彻底由 `ZipCompressionProfile.forLevel()` 替代，实现强类型透明。
- **C-002**: 性能门禁如何划分？
  - 答：Store >= 5000 MB/s, L1 >= 3000 MB/s, L2/L3 >= 3000 MB/s, L4 >= 1500 MB/s, L5 >= 200 MB/s, L6 >= 2.0 MB/s, L7 >= 0.20 MB/s (Debug 模式)。
