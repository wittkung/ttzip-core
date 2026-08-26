# Implementation Plan: 060-arm64-pmull-crc64-acceleration

**Feature Name**: ARM64 PMULL 硬件级 CRC64 (ECMA-182) 加速引擎接入  
**Status**: Planned  
**Created**: 2026-08-17  
**Feature Spec**: [spec.md](./spec.md)

---

## 1. Technical Context

- **目标**: 将在 xz-upstream / liblzma 验证的高性能 ARM64 PMULL 向量折叠与 Barrett 模约化算法实装至 TTZip。
- **涉及模块**:
  - `Sources/CTTZipBridge/include/ttzip_crc64.h`
  - `Sources/CTTZipBridge/ttzip_crc64.c`
  - `Sources/CTTZipBridge/include/module.modulemap`
  - `Sources/CTTZipBridge/include/CTTZipBridge.h`
  - `Sources/TTZipCore/Crypto/CRC64Checksum.swift`
  - `Tests/TTZipTests/CRC64HardwareTests.swift`
- **指令集**: ARMv8-A NEON + Crypto (`vmull_p64`)
- **生成多项式**: ECMA-182 反转形式 `0xC96C5795D7870F42ULL`
- **常量规范**:
  - `fold512` = `(0x081f6054a7842df4, 0x6ae3efbb9dd441f3)`
  - `fold128` = `(0xdabe95afc7875f40, 0xe05dd497ca393ae4)`
  - `mu_p`    = `(0x9c3e466c172963d5, 0x92d8af2baf0e1e84)`
- **黄金预言机**: ASCII `"123456789"` 的 CRC64 (ECMA-182) 比特精确等于 `0x6C40DF5F0B497347ULL`

---

## 2. Constitution & Invariants Check

- [x] **Zero-Cost Abstraction on Hot Paths**: CRC64 计算为纯无锁、无堆分配、无中间对象的原地流式与缓冲区运算。
- [x] **Zero Zero-Fill Faults**: Swift 封装直接借用 `Data.withUnsafeBytes` 内存裸指针，杜绝 `Data(count:)` 内核清零。
- [x] **Fast-Path Bypass Preservation**: Apple Silicon 原生 ARM64 平台直接内联 `ttzip_crc64_pmull`，绝不走通用标量慢路径。
- [x] **Bounds-First**: 0 字节与空指针校验在 C 与 Swift 两层均有确定性确界防护，窄化传参使用 `size_t`。
- [x] **Oracle-First**: 引入标准 ASCII 黄金向量、0~256 字节穷举差分预言机与 10MB 硬件吞吐压测门禁（$\ge 30,000\text{ MB/s}$）。

---

## 3. Phase 0: Research Index

- - R001 [SUBAGENT:research] 《ARM64 PMULL 向量折叠与 Barrett 模约化数学正确性》：已完成，常量与黄金向量均通过交叉验证。
- - R002 [SUBAGENT:research] 《非 ARM64 平台标量 Slicing-by-8 查表 Fallback》：已完成，16KB 静态表保证 L1D Cache 驻留与跨架构比特一致性。
- - R003 [SUBAGENT:research] 《Swift 零拷贝适配与 Modulemap 模块集成》：已完成，使用 `@inlinable` 零开销封装与 `module.modulemap` 导出。

详细调研记录见 [research.md](./research.md)。

---

## 4. Phase 1: Design & Contracts Index

- **数据模型**: [data-model.md](./data-model.md)
- **强类型契约**:
  - `contracts/crc64_c_bridge_contract.json`
  - `contracts/crc64_swift_checksum_contract.json`
  - `contracts/crc64_benchmark_gate_contract.json`
- **快速验证指南**: [quickstart.md](./quickstart.md)

---

## 5. Component Changes & Architecture

### CTTZipBridge (C 底层桥接层)
- **[NEW] `Sources/CTTZipBridge/include/ttzip_crc64.h`**:
  - 导出 `ttzip_crc64(const uint8_t *buf, size_t size, uint64_t crc)`
  - 导出 `ttzip_crc64_pmull(const uint8_t *buf, size_t size, uint64_t crc)`
- **[NEW] `Sources/CTTZipBridge/ttzip_crc64.c`**:
  - ARM64 NEON PMULL 4 路 64 字节向量折叠、16 字节折叠与 Barrett 约化
  - 非 ARM 平台的 Slicing-by-8 标量查表 fallback
- **[MODIFY] `Sources/CTTZipBridge/include/module.modulemap`**:
  - 添加 `header "ttzip_crc64.h"`
- **[MODIFY] `Sources/CTTZipBridge/include/CTTZipBridge.h`**:
  - 包含 `#include "ttzip_crc64.h"`

### TTZipCore (Swift 核心层)
- **[NEW] `Sources/TTZipCore/Crypto/CRC64Checksum.swift`**:
  - 提供 `@inlinable public static func calculate(for data: Data, seed: UInt64 = 0) -> UInt64`
  - 提供 `@inlinable public static func calculate(buffer: UnsafeRawBufferPointer, seed: UInt64 = 0) -> UInt64`

### TTZipTests (测试与门禁)
- **[NEW] `Tests/TTZipTests/CRC64HardwareTests.swift`**:
  - 黄金测试向量校验（`"123456789"` -> `0x6C40DF5F0B497347`）
  - 0 字节至 256 字节穷举差分对比
  - 随机非对齐跨边界切片对比
  - 10MB 缓冲区性能压测门禁（$\ge 30,000\text{ MB/s}$）
