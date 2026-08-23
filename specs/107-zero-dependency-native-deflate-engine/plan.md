# Implementation Plan: 100% 自研零外部依赖原生 Apple Silicon DEFLATE 引擎体系

**Feature ID**: `107-zero-dependency-native-deflate-engine`  
**Status**: IN_PLANNING  
**Target Delivery**: 100% 自研纯 C 原生 Deflate 引擎、彻底剥离 `libdeflate.a` 与 `<zlib.h>`、18 核心并发分块与 32KB 跨 Tile 字典预热  

---

## 1. Technical Context & Principles

### 1.1 核心架构目标与设计原则
1. **100% 自主可控纯 C 源码**：在 `Sources/CTTZipBridge/native_deflate/` 下实现原生 LZ77 匹配查找器、Canonical Huffman 树构建器、64 位快速位流累加器与统一调度引擎；
2. **彻底解耦外部库**：编译与运行时零调用 `libdeflate.a`、零调用系统 `<zlib.h>` / `libz.dylib`；
3. **Apple Silicon 硬件微架构深度适配**：
   - ARM64 NEON SWAR 64-bit/128-bit 向量化匹配展开；
   - ARM64 `rbit + clz` 单周期比特反转与前导零快速计算；
   - 128KB L1D 缓存驻留的 2-Way 哈希表，热路径零堆分配；
4. **18 核心 Tile 饱和并发与 32KB 跨块字典预热**：
   - 前 $N-1$ 块输出 `BFINAL=0` 与 RFC 1951 `Z_SYNC_FLUSH` 对齐标记，末尾块输出 `BFINAL=1`；
   - 非首块 Tile 传入前一块末尾 32KB 历史数据，初始化负偏移哈希表，彻底消除边界压缩率断层。

---

## 2. Phase 0: Research Items (`research.md` 索引)

- R001 [SUBAGENT:research] 《ARM64 NEON 硬件矢量化 LZ77 匹配查找器与 Hash4/Hash3 算法设计》：L1D 128KB 2-Way 表、SWAR 64-bit 单词比对与 `vqaddq_s16` 饱和重定位
- R002 [SUBAGENT:research] 《Canonical Huffman 树受限码长生成与 64-bit 高速位流累加器设计》：In-Place 2-Queue 双队列构建、15-bit 浅叶借位、RLE 游程编码与 64-bit 无分支位累加器
- R003 [SUBAGENT:research] 《100% 自研原生 Deflate 架构与 18 核心 Tile 并发编排拓扑》：`native_deflate/` 纯 C 模块化设计、32KB 跨 Tile 字典预热与外部静态库彻底剥离

---

## 3. Phase 1: Contracts, Data Models & Validation

- `data-model.md`: 定义 `ttzip_native_deflate_options_t`, `ttzip_bitstream_t`, `ttzip_deflate_fast_mf_t`, `ttzip_deflate_lazy_mf_t`, `ttzip_huffman_codes_t` 等实体。
- `contracts/native-deflate.schema.json` [SUBAGENT:research]: 定义原生 Deflate 压缩配置与分块结果契约。
- `quickstart.md`: 快速执行单元测试、100MB 真实语料测试与 `/usr/bin/unzip -t` 校验指令。

---

## 4. Proposed Changes by Component

### Component 1: `Sources/CTTZipBridge/native_deflate/` (自研纯 C 原生 Deflate 引擎)
- [NEW] `ttzip_deflate_bitstream.h`: 64 位无分支寄存器位流累加器与 RFC 1951 字节对齐
- [NEW] `ttzip_deflate_huffman.h` & `ttzip_deflate_huffman.c`: 静态/动态 Canonical Huffman 树构建与 RLE 头部编码
- [NEW] `ttzip_deflate_fast.c`: Tier 1/2 极速匹配查找器（128KB 2-Way L1D 驻留表 + SWAR 64-bit 比对）
- [NEW] `ttzip_deflate_lazy.c`: Tier 3/4 Lazy 延迟判定匹配查找器（Hash3/Hash4 双表 + 1-Step Lazy Evaluation）
- [NEW] `ttzip_deflate_engine.h` & `ttzip_deflate_engine.c`: 统一分发中枢、32KB 跨 Tile 字典预热与 Zopfli 衔接

### Component 2: `Sources/CTTZipBridge/` (C 桥接层统一重构)
- [MODIFY] `ttzip_zopfli_engine.c`: 移除 `<zlib.h>` 与 `libdeflate.h`，全面直通自研 `ttzip_native_deflate`
- [MODIFY] `CTTZipBridge_ZipWrite.c` & `CTTZipBridge_ZipChunkedStream.c`: 切换至自研原生 Deflate 压缩内核
- [MODIFY] `CTTZipStreamCoder.c`: 移除外部 `libdeflate` 分配器与压缩函数，接入自研原生流式接口

### Component 3: `Sources/TTZipCore/Zip/` (Swift 调度与管线接入)
- [MODIFY] `ZipExtremeBlockWriter.swift`: 调用自研原生 Deflate 接口进行 18 核心 Tile 并发压缩与 32KB 字典预热
- [MODIFY] `ZipParallelWriter.swift`: 调用自研原生 Deflate 接口进行多文件并行写入

### Component 4: `Tests/TTZipTests/` (全面验证与回归)
- [NEW] `NativeDeflateEngineTests.swift`: 覆盖单字节、小文件、10MB 与 100MB 语料自研 Deflate 编解码断言
- [MODIFY] `ZipExtremeBlockWriterTests.swift`: 验证自研原生 Deflate 引擎与 `/usr/bin/unzip -t` 100% 零错误
- [MODIFY] `ZipMultiCoreParetoFrontierPkTests.swift`: 全量实测自研引擎在 18 核心满载下的吞吐与体积
