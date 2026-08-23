# Feature 033: 全矩阵清零持平、波动与倒退并全面大幅跃升 (Full-Matrix Leapfrog Zero-Flat Closure)

## 1. 业务目标与第一性原理 (Business Goals & First Principles)

- **核心动机**：
  在全格式 16 种格式 1v1 PK 基准测试中，杜绝任何仅仅满足于“持平（±3%）”或“常态微幅波动（-3%~-10%）”的妥协，以历史最优性能纪录为唯一门禁底线，针对单流式格式（LZ4、LZIP、LRZIP、TAR.XZ）和加密分发路径实施纯 C 原生进程内通道重构，彻底根除外部进程 `fork() + execve()` 开销，实现全矩阵持平项、波动项和倒退项的全面清零与全绿大幅领先。

- **成功标准 (Success Criteria)**：
  1. **零持平、零波动、零倒退**：对比基准 `071939` 与峰值矩阵，全量 246 项细分维度中，持平项、波动项和倒退项数量清零，提升项占比推升至最高。
  2. **LZ4 / LZIP / LRZIP / TAR.XZ 全场景大幅突破**：10MB 拟真日志与海量小文件打包解压耗时降低至 $\le 2.5\text{ ms}$（吞吐 $\ge 3,500\text{ MB/s}$）。
  3. **AES-256 加密解压全场景大幅突破**：7Z 与 DMG 加密解压直通 ARM NEON SIMD 原生通道，吞吐恢复并稳定在 $\ge 8,500\text{ MB/s}$。
  4. **全量 593+ 单元测试 100% 绿灯通过**。

---

## 2. 用户故事与验收场景 (User Stories & Scenarios)

### User Story 1: LZ4 / LZIP / LRZIP 进程内纯 C 原生通道接入 (Priority: P1) 🎯 MVP

- **场景**：用户使用 TTZip 压缩或解压 LZ4、LZIP、LRZIP 归档。
- **行为**：TTZip 通过进程内纯 C 动态静态库绑定执行编解码，杜绝任何外部进程 `fork()` 调用。
- **验收断言**：10MB 日志打包解压耗时 $\le 2.5\text{ ms}$，吞吐 $\ge 3,500\text{ MB/s}$。

### User Story 2: TAR.XZ 内存流式原生管道与零拷贝 (Priority: P1)

- **场景**：用户打包或解压 `.tar.xz` 或 `.xz` 归档。
- **行为**：TTZip 通过 liblzma 纯 C 内存流直接对接 TAR 管道，消除子进程开销。
- **验收断言**：TAR.XZ 10MB 日志打包解压吞吐恢复至 $\ge 800\text{ MB/s}$。

### User Story 3: AES-256 加密解压直通原生 ARM NEON SIMD 引擎 (Priority: P1)

- **场景**：用户解压含有密码保护的 7Z 或 DMG 归档。
- **行为**：`ArchiveExtractor+Dispatch.swift` 优先路由至 `SevenZipEngine`（ARM NEON SIMD AES-256），避免回退至慢速通用 C 管道。
- **验收断言**：7Z / DMG 100MB AES 解压吞吐稳定在 $\ge 8,000\text{ MB/s}$。

---

## 3. 功能性需求与非功能性需求 (Requirements)

### Functional Requirements
- **FR-01**：在 `ttzip_create_tar_native.c` 与 `TarArchiveEngineTemplate.swift` 中，针对 LZ4 / LZIP / LRZIP 实施纯 C 进程内直通，禁止调用外部二进制命令行。
- **FR-02**：在 `ttzip_tar_native.c` 中为 XZ 压缩挂接 liblzma 内存流写入器，消除 `fork()`。
- **FR-03**：在 `ArchiveExtractor+Dispatch.swift` 中强化针对 DMG 加密归档的分发逻辑，当 `password != nil` 时直通 `SevenZipEngine`。

### Non-Functional Requirements
- **NFR-01 (性能零倒退铁律)**：全格式 46 项基准测试 246 个细分维度全部居于历史最优设定，任何维度严禁发生 $\Delta < -10.0\%$ 倒退。
- **NFR-02 (内存安全)**：C 桥接层所有内存缓冲区必须通过 `CUnsafeBufferAdapter` 严格生命周期管理，零内存泄漏。
- **NFR-03 (严格日志纪律)**：所有模块严禁裸 `print` 或 `printf`，统一经由 `TTLogger` 统一调度。
