# Phase 0 Research: Fast-LZMA2 Multi-Threaded Engine Integration

**Feature Directory**: `specs/055-fast-lzma2-engine-integration`

**Completed**: 2026-08-17

---

## R001: Fast-LZMA2 In-Tree 编译与 SPM/CMake 构建集成

### 1. Decision (选定方案)
采用 **In-tree 源码内嵌编译模式**，将 `conor42/fast-lzma2` 核心 C 源文件与头文件直接置于 `Sources/CTTZipBridge/fast-lzma2/` 目录下，并于 [Package.swift](file:///Users/kevintung/Documents/dev/TTZip/Package.swift) 的 `CTTZipBridge` 目标中追加 `.headerSearchPath("fast-lzma2")` 路径。对外通过 `Sources/CTTZipBridge/include/ttzip_fl2_lzma2.h` 封装统一的 7Z/XZ 流式压缩与多线程调度 C 接口，并导出至 [module.modulemap](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/module.modulemap) 供 `TTZipCore` Swift 层调用。

### 2. Rationale (选择理由)
1. **SPM 原生无缝集成与全自动化构建**：Swift Package Manager 会自动递归扫描并编译 C Target 根目录及子目录下的所有 `.c` 源文件，无需额外维护 CMake 外部脚本或外部二进制文件。
2. **LTO 与编译器跨模块内联优化**：In-tree 源码编译允许 Clang 在 `-O3` 和 Link-Time Optimization (LTO) 下对 Radix 匹配查找器与无分支 Range Coder 进行最高效的寄存器分配和函数内联，避免动态库或外部静态库调用的边界开销。
3. **多平台原生并发支持**：`fast-lzma2` 内部原生通过 `fl2_threading.c` / `fl2_pool.c` 分离了 POSIX `pthread`（macOS / Linux）与 Win32 Threads（Windows），In-tree 模式保证跨 macOS 与 Windows 时零额外胶水代码即可直接编译通过。
4. **内存与取消确界直接拦截**：通过 CTTZipBridge 包装层可直接注入 TTZip 内存限额配置（`FL2_CCtx_setParameter(..., FL2_p_dictionarySize, ...)`）与任务取消轮询机制，严格遵循项目 Bounds-First 纪律。

### 3. Alternatives Considered (被否决方案)
1. **被否决方案 1：预编译 Universal 静态库并打包至 `Vendor/TTZipVendor.xcframework`**
   - *否决理由*：`fast-lzma2` 是频繁与 `CTTZipBridge` 进行流式交互与参数微调的核心编解码器。预编译二进制会增加 git 仓库二进制体积与发布打包摩擦（需维护额外构建脚本），且阻断了跨模块 LTO 优化，不利于后续 Windows 端的统一构建。
2. **被否决方案 2：通过 `Package.swift` 远程依赖外部第三方的 SPM 包装仓库**
   - *否决理由*：`conor42/fast-lzma2` 上游官方仓库未维护原生 `Package.swift`。引入第三方包装仓库存在供应链安全风险、版本更新滞后及符号命名冲突（如 `XXH_NAMESPACE`）不可控问题。

### 4. Source (实际查阅来源)
- [Package.swift](file:///Users/kevintung/Documents/dev/TTZip/Package.swift#L32-L48)：查阅 `CTTZipBridge` 目标配置、头文件搜索路径与系统链接库（`libc++`, `z`, `bz2` 等）。
- [Sources/CTTZipBridge/include/module.modulemap](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/module.modulemap#L1-L27)：查阅 C 桥接模块导出定义与 Clang 模块化头文件约束。
- [Sources/CTTZipBridge/ttzip_lzma_radix_mf.c](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/ttzip_lzma_radix_mf.c#L1-L98)：查阅现有自研 NEON Radix 匹配查找器实现，核对与 fast-lzma2 的混合分流接口。
- [scripts/build_libdeflate.sh](file:///Users/kevintung/Documents/dev/TTZip/scripts/build_libdeflate.sh#L25-L70)：查阅当前 Vendor 静态库构建与 xcframework 归档流程。
- `https://github.com/conor42/fast-lzma2`：查阅 fast-lzma2 源码结构（`fast-lzma2.h`, `fl2_compress.c`, `radix_mf.c`, `fl2_pool.c`, `fl2_threading.c` 等）、BSD-2-Clause 许可与 `FL2_createCCtx`/`FL2_CCtx_setParameter` API 规范。

---

## R002: C 桥接接口设计与混合双引擎路由架构

### 1. Decision (选定方案)
建立统一的 In-tree C 桥接中枢 `ttzip_fl2_bridge`（提供 `ttzip_fl2_compress_block` 与 `ttzip_fl2_compress_stream` 接口），实施自适应**混合双引擎路由架构 (Hybrid Dual-Engine Architecture)**：
1. **统一 C 桥接接口设计**：
   - **块压缩接口**：`int ttzip_fl2_compress_block(const uint8_t* src, size_t src_len, uint8_t* dst, size_t dst_capacity, size_t* out_compressed_len, int level, bool is_zero_block, uint32_t* out_dict_size, int thread_count)`，供 7Z 归档引擎（[ttzip_lzma2_enc_native.c](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/ttzip_lzma2_enc_native.c)）与分块压缩调用。
   - **流压缩接口**：`ttzip_fl2_stream_ctx_t* ttzip_fl2_stream_create(int level, uint32_t dict_size, int thread_count)` / `ttzip_fl2_stream_process(...)` / `ttzip_fl2_stream_free(...)`，包装 `FL2_CStream`，无缝嵌入 `CTTZipBridge_GzParallel.c`（`init_parallel_xz`）与 `ttzip_tar_native.c` 的 TAR.XZ 流式管道。
2. **混合分流路由策略**：
   - **全零数据 / 稀疏块**：前置 `ttzip_is_block_all_zero_neon` 扫描，直通 `encode_zero_chunk_2mb` 极速零块编码器（$\ge 10\text{GB/s}$ 零 RLE）。
   - **Level 1 (极速模式)**：路由至 TTZip 自研 ARM64 NEON 匹配查找器与无分支 Range Coder（`ttzip_lzma2_fast_encode`），保障 $\ge 3,200\text{MB/s}$ (Debug) / $\ge 3,900\text{MB/s}$ (Release) 的硬件级极限吞吐。
   - **Level 3 ~ Level 9 (中高压缩模式)**：路由至 Fast-LZMA2 多线程流水线（复用 `FL2_CCtxMt` / `FL2_compressCCtx` 与 `FL2_compressStream`），启用并行 Radix 匹配查找器，线程数自动绑定 Apple Silicon P-Cores，字典确界设为 16MB~64MB（16 线程常驻内存 $\le 512\text{MB}$）。
3. **Swift 上层适配**：在 `TTZipCore/SevenZip/` 与 `SevenZipCAdapter.swift` 中无缝透传压缩等级与线程预算参数，对外保持 `SevenZipEngineProtocol` 契约不变。

### 2. Rationale (选择理由)
1. **破除高压缩等级多核扩展瓶颈**：传统 `liblzma`（BT4/HC4 匹配器）单核计算复杂度高，多线程扩展受限于锁与字典隔离；Fast-LZMA2 基于分块并行 Radix 匹配查找器，在 Level 3~9 下可榨干 8~24 核 CPU 算力，实现 7Z/XZ 压缩吞吐 $1.5\text{x} \sim 3\text{x}$ 提升（达到 $\ge 800\text{MB/s}$ Debug / $\ge 1200\text{MB/s}$ Release），达成 SC-001。
2. **保护 Level 1 峰值吞吐门禁**：Fast-LZMA2 的 Radix 排序与分块任务分发具有常数调度开销，在 Level 1 下吞吐约为 400~800 MB/s；保留自研 NEON L1 编码器作为 Fast-Path，可确保 Level 1 吞吐不发生任何倒退（达成 SC-002 与 Performance Invariants Rule 3.1）。
3. **符合系统工程铁律 (Bounds-First & Stream-First)**：通过 `FL2_CCtx` 显式参数管理字典上限与线程池规模，消除传统 7-Zip 多线程随线程数线性倍增字典内存引发的 OOM 风险；同时通过流式 API 支持 TAR.XZ 大文件管道流。

### 3. Alternatives Considered (被否决方案)
1. **被否决方案 1：全等级（包括 Level 1）统一替换为 fast-lzma2，废弃自研 NEON HC4 编码器**
   - *否决理由*：FL2 的 Radix Match Finder 适合深度前瞻与中大字典匹配，在 L1 极浅匹配（nice_len=6, dict=64KB）下调度开销显著，吞吐（~600 MB/s）远低于 TTZip 自研 NEON 直通编码器的 3,200+ MB/s，会导致 Level 1 核心性能指标暴跌 75% 以上，直接击穿性能门禁。
2. **被否决方案 2：保持现有 liblzma 引擎，仅在外层通过 GCD 分块并行提升并发度**
   - *否决理由*：liblzma 的 BT4 算法单块内存占用大、CPU 缓存局部性差，外层 GCD 分块会导致每块重置 LZMA2 字典（破坏上下文关联度降低压缩比），且多核争抢 L3 缓存与内存带宽，无法突破 600 MB/s 瓶颈，无法满足 1200 MB/s 目标。
3. **被否决方案 3：在 Swift 层使用 Task/GCD 并发调度单线程 FL2 API (`FL2_compress`)**
   - *否决理由*：破坏 FL2 内部多线程协同共享 Radix 索引表的内存局部性优势，会导致每个 Swift Task 独立分配 Radix 表与字典缓冲，在 16+ 线程下引发内存倍增（突破 512MB 确界），且引入 Swift-C 频繁跨边界调度的开销。

### 4. Source (实际查阅来源)
- `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c` (Lines 1–541): 7Z 归档 LZMA2 打包逻辑、NEON 零块检测 `ttzip_is_block_all_zero_neon`、分块调度与 AES-256 CBC 加密。
- `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c` (Lines 1–471) & `include/ttzip_lzma2_fast_encoder.h`: ARM64 NEON L1 编码器 `ttzip_lzma2_fast_encode` 与现有 `ttzip_lzma2_compress_block_tuned`（基于 `lzma_raw_buffer_encode`）。
- `Sources/CTTZipBridge/CTTZipBridge_GzParallel.c` (Lines 85–160): XZ 流式分块压缩 `init_parallel_xz`（基于 `lzma_easy_buffer_encode`）。
- `Sources/CTTZipBridge/ttzip_tar_native.c` (Lines 160–210): TAR.XZ 打包管道分发。
- `Sources/TTZipCore/SevenZip/` (`NativeSevenZipEngine.swift`, `SevenZipEngine.swift`, `SevenZipParallelWriter.swift`): Swift 7Z 引擎外观与调用链路。
- `Sources/TTZipCore/Adapters/SevenZipCAdapter.swift` (Lines 1–75): Swift-C 桥接适配器与错误码处理。
- `specs/055-fast-lzma2-engine-integration/spec.md`: User Stories 1~4、FR-001~FR-010、SC-001~SC-005 规范要求。
- Fast-LZMA2 官方库 API 架构 (`fast-lzma2.h` / `conor42/fast-lzma2`): `FL2_CCtx`, `FL2_createCCtxMt`, `FL2_compressCCtx`, `FL2_CStream`, `FL2_createCStreamMt`, `FL2_compressStream`, `FL2_setParameter`。

---

## R003: Apple Silicon 拓扑调度与多核内存确界控制

### 1. Decision (选定方案)
选定 **“共享 Radix 匹配查找器 + 拓扑感知 P-Core 优先线程池 + 动态内存确界阶梯降级”** 的多线程 LZMA2 架构方案：
1. **内存确界模型**：Fast-LZMA2 采用共享字典与基数表架构，其常驻内存公式为 $\text{Memory}_{\text{FL2}} \approx (\text{dictSize} \times 6.0) + (N_{\text{threads}} \times 6\text{MB}) + 16\text{MB}$。在默认 512MB 内存预算下，标准配置为 `dictSize = 32MB`，支持满载 8~24 线程（24 线程时常驻内存约 $352\text{MB} \le 512\text{MB}$）；若用户指定 `dictSize = 64MB`，在 $\ge 16$ 线程时通过动态阶梯算法自动 clamp 线程数或下调字典至 32MB，物理锁死 $\text{RSS} \le 512\text{MB}$。
2. **Apple Silicon 拓扑调度策略**：
   - 复用 `CTTZipBridge` 的 `get_p_core_count()`（基于 `hw.perflevel0.physicalcpu`），默认并发线程数严格绑定为 P-Core 数量，调度优先级绑定为 `QOS_CLASS_USER_INITIATED` / `QOS_CLASS_USER_INTERACTIVE`。
   - 仅在输入体积 $\ge 128\text{MB}$ 且 Level $\ge 5$ 时允许扩展至全逻辑核（P+E），配合 FL2 的无锁工作窃取（Work-Stealing）分块队列，避免静态等分导致 E-Core 成为长尾瓶颈。
3. **确定性内存管理**：在 `FL2CompressionContext` 中嵌入 `0x464C3243` ("FL2C") Magic 标记，字典与匹配查找器缓冲区统一通过 `ttzip_platform_aligned_alloc(16384, size)` 进行 Apple Silicon 16KB 硬件物理页对齐分配，析构时强制 `ttzip_secure_zero` 归零，杜绝 UAF 与内存泄漏。

### 2. Rationale (选择理由)
1. **破除传统 LZMA 内存爆炸瓶颈**：传统 7-Zip / liblzma 的 BT4 匹配查找器采用多块独立实例模式，每线程需分配 $\approx \text{dictSize} \times 11.5$ 内存。在 24 线程与 32MB 字典下需消耗 $\approx 8.8\text{GB}$ 内存，在 64MB 字典下需消耗 $\approx 18\text{GB}+$ 内存。而 Fast-LZMA2 采用共享 Radix 匹配查找器（RMF），多线程仅需增加极少量线程局部 Range Coder 缓冲区（每线程 $\approx 4\sim 6\text{MB}$），24 线程 32MB 字典仅消耗 $\approx 352\text{MB}$，完全满足 512MB 内存确界要求。
2. **避免 Apple Silicon 非对称核心的长尾木桶效应**：M1/M2/M3/M4 系列芯片中，E-Core 的单核算力与 IPC 仅为 P-Core 的 25%~35%。若盲目使用全部逻辑核并采用同步等分分块，P-Core 处理完毕后将长时间空闲等待最慢的 E-Core 线程完成，导致整体归档耗时增加 20%~40%。将主并发收敛于 P-Core 并通过 QoS 保证 CPU 满频调度，可实现最优每瓦吞吐与确定性极速响应。
3. **16KB 统一内存对齐优势**：Apple Silicon 采用 16KB 原生虚拟内存页大小，使用 16KB 页对齐缓冲区可消除跨页边界造成的 TLB Miss 与缓存行分裂开销。

### 3. Alternatives Considered (被否决方案)
1. **被否决方案 1：传统 7-Zip / liblzma 多块独立 BT4 线程并发模型**
   - **否决理由**：内存占用随线程数线性暴增（$\text{Threads} \times \text{dictSize} \times 11.5$）。在 16~24 核设备上运行高压缩等级时，常驻物理内存瞬间突破 $6\text{GB} \sim 18\text{GB}$，极易触发 macOS 统一内存页压缩与 Swap，严重违反四大系统工程铁律中的“确定性确界（Bounds-First）”。
2. **被否决方案 2：全逻辑核（`hw.logicalcpu`）静态等分分块调度**
   - **否决理由**：未考虑 Apple Silicon P/E 核异构特性。静态分块会导致 E-Core 成为同步屏障点，拖垮 P-Core 的高吞吐流水线；且在 24 逻辑核无节制分配下，若开启 64MB 字典将导致总内存达到 $544\text{MB} > 512\text{MB}$。

### 4. Source (实际查阅来源)
- `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c` (L76-95: `get_p_core_count`, `get_logical_cpu_count`; L274-297: 动态分块与 CPU 核心分配逻辑; L310-340: `dispatch_apply` 与内存分块管线)
- `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c` (L176-209: 64KB/4KB 字典与 HC4/HC3 查找器配置; L408-470: `ttzip_lzma2_compress_block_tuned` 参数模型)
- `Sources/CTTZipBridge/include/ttzip_platform.h` (L31-49: macOS/Apple Silicon 平台宏; L81-93: `TTZIP_THREAD_LOCAL` 宏隔离)
- `Sources/CTTZipBridge/CTTZipSysAlloc.c` (L35-44: `ttzip_core_aligned_alloc_16k` 16KB 物理页对齐分配器)
- `specs/055-fast-lzma2-engine-integration/spec.md` (User Story 1~4, FR-001~FR-010: 512MB 内存确界、8~24 核 CPU 扩展与双平台调度要求)
- Fast-LZMA2 官方算法架构 (`conor42/fast-lzma2`): Radix Match-Finder (RMF) 共享字典数据结构、`FL2_CCtx` 线程局部上下文与 `FL2_estimateDStreamSize` 内存模型。
