# Feature Specification: Comprehensive Optimization Default Assembly (全量优化技术默认装配与透明解耦中枢)

**Feature Directory**: `specs/092-comprehensive-optimization-default-assembly/`  
**Status**: DRAFT  
**Priority**: P1  
**Created**: 2026-08-18  

---

## Executive Summary

本项目在近期研发中成功落地了大量软硬件协同与算法优化技术（包括 ARM64 PMULL CRC-64 硬件加速、SWAR 64B 高速扫描、4-Way NEON Adler-32、SWAR 八进制与 512B 校验和、Bit-Grooming 浮点精密量化、NEON BitShuffle/ByteDelta 向量重排、特殊值 uniform block 总线旁路、128KB L1D 微块两级分块共享字典、微块懒加载切片以及 Shannon 熵级联自适应调优器）。

本规范的目标是：**贯彻“反配置膨胀与默认透明行为 (Zero Configuration Creep)”工程铁律，将上述所有优化技术在通用归档流水线（`ArchiveWriter`、`BaseArchiveEngineTemplate`、`TarArchiveEngineTemplate`）与综合性能评测套件（`CompetitorBenchmarkRunner`、`AllFormatsPkSuiteTests`）中全面默认装配与透明启用**，实现零用户配置心智负担下的全场景性能自动爆发。

---

## User Scenarios & Functional Requirements

### User Story 1 (P1): 通用归档流前置自适应启发式探针透明装配 (Transparent Adaptive Heuristics)
- **As a** 最终用户或上层应用程序 (GUI / CLI)，
- **I want to** 在调用通用的归档与压缩接口时无需手动判断数据类型，
- **So that** 引擎内部自动对不可压数据（高熵图片/视频/已压缩包）进行 0 耗时直通存储，对全零/稀疏块进行总线级旁路，彻底消除负压缩与 CPU 周期浪费。

#### Functional Requirements:
1. `FR1.1`: 在 `BaseArchiveEngineTemplate.prepareEnvironment` 与 `ArchiveWriter+Dispatch` 中透明挂载 `AdaptivePipelineOrchestrator`。
2. `FR1.2`: 针对 $\ge 64\text{KB}$ 的待压缩数据流，自动执行 16KB 微采样 Shannon 熵与均匀性探测 (`ttzip_heuristic_eval_cascade`)。
3. `FR1.3`: 若判定为高熵不可压（$H > 7.65$），当前文件/数据块自动透明降级为 `Store / Direct` 存储，免去 Deflate/Zstd 编码开销。
4. `FR1.4`: 若判定为全零或单字节常数块，自动标记为特殊值块或跳过压缩计算。

---

### User Story 2 (P1): 科学浮点自相关性识别与 Bit-Grooming 级联 (Auto-Detected Scientific Float)
- **As a** 科学计算、传感器数据或金融时序数据处理用户，
- **I want to** 在归档包含大量 Float32/Float64 数组的文件时，
- **So that** 引擎通过步长自相关性探针（Stride Autocorrelation $\ge 0.85$）自动识别浮点特征，并联动 Bit-Grooming + BitShuffle，实现空间压缩比翻倍。

#### Functional Requirements:
1. `FR2.1`: `AdaptivePipelineOrchestrator` 在微采样阶段计算 4 字节 / 8 字节步长自相关系数。
2. `FR2.2`: 当浮点相关性显著且配置为可自适应精度模式时，透明挂载 `ttzip_filter_bitgroom_float32_neon` 或 `ttzip_filter_bitgroom_float64_neon`。
3. `FR2.3`: 确保相对误差严格受控于 $\le 0.5 \times 10^{1 - NSD}$。

---

### User Story 3 (P1): 综合性能基准与大满贯 PK 矩阵全量装配 (Benchmark Full-Stack Wiring)
- **As a** 性能测试套件与 CI/CD 自动化流水线，
- **I want to** 在 `CompetitorBenchmarkRunner` 与 `AllFormatsPkSuiteTests` 中全量体现所有软硬件协同优化收益，
- **So that** 竞品 1v1 PK 矩阵在全 16 种格式与真实/科学/稀疏混合数据集下均达到物理最优状态。

#### Functional Requirements:
1. `FR3.1`: `CompetitorBenchmarkRunner` 默认启用自适应流水线编排与硬件加速校验（PMULL CRC64, Adler-32 NEON）。
2. `FR3.2`: 在综合 PK 矩阵中增加浮点科学矩阵与稀疏数据集专用评测项，验证端到端加速比与压缩率增益。
3. `FR3.3`: 在 `XCTestPerformanceMeasureTests` 中固化自适应调优与浮点量化的硬件性能门禁。

---

### User Story 4 (P2): 统一全局透明配置与架构解耦治理 (Architecture Governance)
- **As a** 软件架构师，
- **I want to** 确保所有新装配的自适应与旁路策略严格遵循 Strategy 与 Template Method 模式，
- **So that** 核心数据路径保持零堆分配、零锁争用、零全局单例状态污染。

#### Functional Requirements:
1. `FR4.1`: 构建 `AdaptivePipelineOrchestrator`，统一收敛自适应策略分发，杜绝在业务代码中硬编码分支。
2. `FR4.2`: 维持 C 桥接层与 Swift 层的清晰解耦，所有 C 结构体在栈上或线程局部初始化。

---

## Success Criteria

1. **高熵数据压缩时延**：对 64KB 随机数据或已压缩媒体文件，归档耗时缩短 $\ge 90.0\%$，体积负膨胀降为 $0\text{ 字节}$。
2. **浮点矩阵压缩增益**：对连续浮点传感器数据，端到端压缩比相比基线提升 $\ge 150.0\%$。
3. **全格式 PK 吞吐**：在 `CompetitorBenchmarkRunner` 全矩阵下保持 100% 格式全面领先或持平竞品，无任何性能倒退。
4. **测试门禁回归**：全量单元测试（535+ 项）100% 通过，13 项硬性能门禁全绿。
