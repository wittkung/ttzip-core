# Feature Specification: 100% 自研零外部依赖原生 Apple Silicon DEFLATE 引擎体系

**Feature ID**: `107-zero-dependency-native-deflate-engine`  
**Status**: DRAFT  
**Created**: 2026-08-19  
**Category**: Core Engine Self-Sufficiency & Algorithmic Conquest  

---

## 1. Executive Summary & Goals

当前 TTZip 在 ZIP 格式的 Deflate 压缩流中，部分依赖了外部预编译静态库 `libdeflate.a` 与系统 `libz.dylib`。尽管通过多核分块取得了领先的基准吞吐，但外部库的存在限制了针对 Apple Silicon 芯片微架构（如 128-byte Cache Line、M4 统一内存总线、NEON 矢量匹配指令与专用寄存器流水线）的深度定制与极致性能释放。

**本特性的核心目标是：全面摆脱外部库依赖，构建 100% 拥有自主可控源码的纯 C 原生 Deflate 引擎体系（`ttzip_native_deflate`）**：
1. **100% 零外部 C 库依赖**：不调用 `libdeflate.a`，不调用 `<zlib.h>`/`libz.dylib`，从 LZ77 匹配查找、Huffman 树生成到 RFC 1951 位流编码器全部由自研 C 源码驱动；
2. **Apple Silicon 硬件原生深度调优**：
   - **ARM64 NEON SWAR 匹配查找器**：利用 64-bit/128-bit 向量化寄存器与 `__builtin_ctzll` / ARM64 `clz` 指令实现单指令多字节比对；
   - **无锁无分配内存设计**：热路径零 `malloc`/`free`，采用预分配栈缓存与线程局部无锁哈希表；
   - **4 级自研匹配解析阶梯**：
     - *Tier 1-2 (Ultra Fast)*: Hash4 单查表 + 贪心解析；
     - *Tier 3-4 (Normal)*: Hash4 + Hash3 双哈希表 + Lazy Evaluation 延迟判定；
     - *Tier 5-6 (Deep Optimal)*: 有限前瞻 DAG 最短路径图论解析器；
     - *Tier 7 (Extreme Peak)*: 多轮香农自信息重平衡迭代 + Katajainen 边界包合并算法。
3. **18 核心 Tile 饱和并发与 32KB 跨块字典预热**：
   - 保留并强化 18 核心并发多 Tile 架构与 32KB 跨块历史字典预热，前 $N-1$ 块输出 `BFINAL=0` 与 `Z_SYNC_FLUSH` 字节对齐，末尾块输出 `BFINAL=1`；
4. **性能与体积双向全面超越**：
   - 在 100MB 真实语料（`enwik8`）下，极速档吞吐 $\ge 6,000\text{ MB/s}$，重压档体积严格超越 `pigz -11` 与 `advzip -4`，系统原生 `/usr/bin/unzip -t` 100% 校验通过（0 错误）。

---

## Clarifications

- **Q1: 零依赖范围是否包含解压（Decompress）？**  
  **A1**: 本阶段重点落地 100% 自研的**原生 Deflate 压缩器与流式编码器**（替换所有 Deflate Write 路径中的 `libdeflate` 与 `zlib`）；解压侧同时支持自研原生解码与现有已调优解压路径，确保双向闭环。
- **Q2: 如何确保生成的 Deflate 比特流严格符合 RFC 1951？**  
  **A2**: 每次编码输出的 ZIP 归档必须通过 macOS 系统自带 `/usr/bin/unzip -t`（0 CRC error）与 `/usr/bin/unzip -p` 逐字节差分验证。
- **Q3: 源码组织结构如何设计？**  
  **A3**: 在 `Sources/CTTZipBridge/native_deflate/` 下组织纯 C 自研模块（`ttzip_deflate_fast.c`, `ttzip_deflate_lazy.c`, `ttzip_deflate_huffman.c`, `ttzip_deflate_bitstream.h`, `ttzip_deflate_engine.c`）。

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: 零外部依赖极速压缩 (Ultra Fast Self-Implemented Deflate)
- **Actor**: 性能工程师 / 普通用户
- **Scenario**: 使用 Tier 1/2 进行极速文件压缩，底层调用自研 `ttzip_deflate_fast`（基于 NEON SWAR Match Finder）。
- **Acceptance Criteria**:
  - 100% 运行自研 C 代码，不发生任何外部库符号调用；
  - 18 核心多块并发吞吐 $\ge 5,000\text{ MB/s}$，压缩包体积优于 `pigz -1`。

### User Scenario 2: 双哈希 Lazy Evaluation 标准压缩 (Balanced Normal Deflate)
- **Actor**: 办公 / 分发用户
- **Scenario**: 使用 Tier 3/4 档位归档文件，底层调用自研 `ttzip_deflate_lazy` 双哈希匹配查找器。
- **Acceptance Criteria**:
  - 18 核心并发吞吐 $\ge 2,500\text{ MB/s}$，产出体积优于 `pigz -6` / `pigz -9` 与 `7z -mx=5`。

### User Scenario 3: 极致重压与系统原生解压验证 (Extreme Peak Squeeze & System Unzip)
- **Actor**: 存储分发管理员
- **Scenario**: 使用 Tier 6/7 档位进行极限重压缩，产出的 ZIP 文件分发至各客户端。
- **Acceptance Criteria**:
  - 产出体积严格 $\le 2.85\text{ MB}$（优于 `pigz -11` 3.01 MB 与 `advzip -4` 2.99 MB）；
  - 系统自带 `/usr/bin/unzip -t` 校验 0 errors。

---

## 3. Functional Requirements

- **FR-001**: 在 `Sources/CTTZipBridge/native_deflate/` 下实现自研 64-bit 快速位流累加器（`ttzip_bitstream.h`），支持批量 64 位整字无条件刷盘。
- **FR-002**: 实现自研 Canonical Huffman 树构建器（`ttzip_deflate_huffman.c`），包含预计算静态 Huffman 表与动态 Huffman 码长受限生成。
- **FR-003**: 实现自研 Fast LZ77 Match Finder（`ttzip_deflate_fast.c`），利用 ARM64 `clz` / NEON SIMD 实现 4-byte 快速哈希索引与单指令最长前缀匹配。
- **FR-004**: 实现自研 Lazy Evaluation LZ77 Match Finder（`ttzip_deflate_lazy.c`），支持 32KB 滑动窗口双哈希表与次优匹配比较。
- **FR-005**: 统一导出 `ttzip_native_deflate_compress_block_with_history` 强类型 C 桥接接口，无缝支持 32KB 跨块字典预热与 `Z_SYNC_FLUSH` 字节对齐。
- **FR-006**: 在 `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift` 与 `ZipParallelWriter.swift` 中全面接入自研原生 Deflate 引擎。

---

## 4. Success Criteria

1. **零库依赖**：所有 Deflate 压缩路径 100% 剥离 `libdeflate.a` 与 `<zlib.h>` 依赖；
2. **基准测试全优**：在 100MB `enwik8` 上，8 大档位在吞吐与体积上全面压制 `pigz`、`7-Zip`、`ouch` 与 `advzip`；
3. **系统解压 100% 合规**：所有压缩包通过 `/usr/bin/unzip -t` 与 `unzip -p` 逐字节差分断言通过。
