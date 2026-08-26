# Feature Specification: 060-arm64-pmull-crc64-acceleration

**Feature Name**: ARM64 PMULL 硬件级 CRC64 (ECMA-182) 加速引擎接入  
**Status**: Clarified  
**Created**: 2026-08-17  
**Module Scope**: `Sources/CTTZipBridge/`, `Sources/TTZipCore/Crypto/`, `Tests/TTZipTests/`

---

## Clarifications

### Session 2026-08-17 (Initial Specification Clarification)
- **Q1: 标量降级路径 (Fallback Strategy)**: 在非 ARM64 架构或不支持 NEON/PMULL 指令集的目标平台上，如何保证计算正确性？
  - **Decision**: 在 `ttzip_crc64.c` 中内置标准的 ECMA-182 标量位反转/查表法实现作为 fallback，确保在任意架构与编译器配置下输出完全比特一致的校验码。
- **Q2: Swift C 模块导出 (Module Visibility)**: 新增的 `ttzip_crc64.h` 如何接入现有的 Swift-C 混编体系？
  - **Decision**: 在 `Sources/CTTZipBridge/include/module.modulemap` 中显式添加 `header "ttzip_crc64.h"`，并在 `CTTZipBridge.h` 中包含引用，使 `TTZipCore` 能够无缝调用 `ttzip_crc64`。
- **Q3: 零长度缓冲区确界 (Zero-Length Invariant)**: 当传入 0 字节长度的 `Data` 或 `NULL` 指针时，系统行为规范为何？
  - **Decision**: 若 `size == 0` 或 `buf == NULL`，直接返回初始 `crc`（或 `seed`），零堆分配、零内存访问、零开销。

---

## 1. User Scenarios & Testing

### User Scenario 1 (US1) - 7Z / XZ 归档与完整性校验硬件加速 [Priority: P1]
- **As a**: TTZip 高性能归档引擎与文件校验核心
- **I want to**: 在执行 7Z / XZ 格式解包、校验或大文件哈希时，使用 ARM64 PMULL (`vmull_p64`) 向量折叠与 Barrett 模约化引擎计算 CRC64 (ECMA-182)
- **So that**: CRC64 校验吞吐从标量查表的 ~1.3 GB/s 跃升至 $\ge 30.0\text{ GB/s}$（达标 48.0 GB/s 峰值），消除归档解压与校验阶段的 CPU 瓶颈。

### User Scenario 2 (US2) - 零拷贝 Swift 安全适配与多粒度流式调用 [Priority: P1]
- **As a**: TTZipCore 业务逻辑开发者与流式数据处理管道
- **I want to**: 通过 `CRC64Checksum.calculate(for:seed:)` 零拷贝传入 `Data` 或裸内存缓冲区
- **So that**: 在 Swift 上层获得与原生 C 语言一致的纳秒级延迟与零堆分配开销。

### User Scenario 3 (US3) - 极端边界全覆盖与跨平台标量兜底 [Priority: P2]
- **As a**: TTZip 跨平台运行环境（Apple Silicon 与 Intel x86_64）
- **I want to**: 对任意字节长度（0 字节、1~7 字节小片段、8~15 字节短缓冲、16~63 字节中等缓冲、$\ge 64$ 字节多块缓冲及非对齐内存）与非 ARM 平台均能产生 100% 比特一致的校验值
- **So that**: 归档校验具备绝对确界的数学一致性与零崩溃容错。

---

## 2. Functional Requirements

- **FR-001**: 在 `Sources/CTTZipBridge/include/ttzip_crc64.h` 中导出 C 原生接口 `ttzip_crc64` 与 `ttzip_crc64_pmull`，遵循 POSIX 与 C11 规范。
- **FR-002**: 在 `Sources/CTTZipBridge/ttzip_crc64.c` 中实现基于 ARM NEON / PMULL (`vmull_p64`) 的 4 路 64 字节向量折叠、16 字节折叠与 Barrett 模约化计算，使用 ECMA-182 生成多项式 `0xC96C5795D7870F42ULL` 与严密派生的 Barrett 约化常量：
  - `fold512` = `(0x081f6054a7842df4, 0x6ae3efbb9dd441f3)`
  - `fold128` = `(0xdabe95afc7875f40, 0xe05dd497ca393ae4)`
  - `mu_p`    = `(0x9c3e466c172963d5, 0x92d8af2baf0e1e84)`
- **FR-003**: 提供标量查表或位运算 fallback，确保在非 ARM64 环境下逻辑正确且比特精确对齐。
- **FR-004**: 在 `Sources/TTZipCore/Crypto/CRC64Checksum.swift` 中实现 Swift 零拷贝封装 `CRC64Checksum`，支持 `Data` 传入与初始种子指定。
- **FR-005**: 黄金校验向量（Golden Vector）：ASCII 字符串 `"123456789"` 的 CRC64 (ECMA-182) 计算结果必须严格等于 `0x6C40DF5F0B497347ULL`（初始种子为 `0`，反转进出）。
- **FR-006**: 在 `Tests/TTZipTests/CRC64HardwareTests.swift` 中建立包含黄金测试向量、0~256 字节穷举差分比对、多切片随机对齐测试及 10MB 吞吐性能门禁的完备测试套件。

---

## 3. Success Criteria

- **SC-001 (数学精确性)**：ASCII `"123456789"` 经 `CRC64Checksum.calculate` 计算产出 `0x6C40DF5F0B497347`，单测 100% 通过。
- **SC-002 (边界确界性)**：0 字节空数据、1~256 字节连续步长数据、非 8/16 字节对齐内存数据的 CRC64 与标量数学基准 100% 比特精确一致。
- **SC-003 (硬件吞吐底线)**：在 Apple Silicon (M-series) 硬件上，10MB 连续内存块的 CRC64 吞吐 $\ge 30,000\text{ MB/s}$（$\ge 30\text{ GB/s}$）。
- **SC-004 (全量工程零倒退)**：`swift test` 全量通过，历史最优性能门禁 `XCTestPerformanceMeasureTests` 零倒退。

---

## 4. Key Entities & Definitions

- **CRC64 ECMA-182**: 标准 64 位循环冗余校验码，生成多项式为 $x^{64} + x^{62} + x^{57} + x^{55} + x^{54} + x^{53} + x^{52} + x^{47} + x^{46} + x^{45} + x^{40} + x^{39} + x^{38} + x^{37} + x^{35} + x^{32} + x^{31} + x^{30} + x^{29} + x^{28} + x^{26} + x^{25} + x^{24} + x^{23} + x^{22} + x^{21} + x^{20} + x^{19} + x^{17} + x^{16} + x^{15} + x^{12} + x^{11} + x^{10} + x^9 + x^8 + x^7 + x^5 + x^4 + x^2 + x^1 + 1$（反射表示 `0xC96C5795D7870F42`）。
- **PMULL (`vmull_p64`)**: ARMv8-A 提供的 64 位无进位多项式乘法指令，单周期完成两路 64 位 GF(2) 乘法。
- **Barrett Reduction**: 无除法多项式模约化算法，利用预计算常量在 $\mathcal{O}(1)$ 周期内将 128 位折叠结果约化为 64 位 CRC 余数。

---

## 5. Assumptions & Dependencies

- 假设系统构建环境为 macOS 14+ (Xcode 15+/16+，Swift 6.0 编译器)。
- 假设在 Apple Silicon ARM64 架构下，编译器默认开启 NEON 与 Crypto/PMULL 指令支持（无需额外 `-march` flag，Clang 默认支持 `vmull_p64`）。
- 依赖 `Sources/CTTZipBridge/include/module.modulemap` 导出 `ttzip_crc64.h` 供 Swift 模块访问。
