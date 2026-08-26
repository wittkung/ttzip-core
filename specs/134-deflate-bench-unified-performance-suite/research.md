# Phase 0 Technical Research: Deflate-Bench Unified Performance & Test Suite Modernization

**Feature Directory**: `specs/134-deflate-bench-unified-performance-suite`  
**Target Subject**: zlib-ng 8 大语料逆向、纯内存 50 点秒级基准引擎与测试体系瘦身重构  

---

## Executive Summary
针对 TTZip 现存基准测试体系碎片化、磁盘 I/O 深度依赖、部分测试用例未加环境变量门禁导致常规 `swift test` 耗时长达 20+ 秒以及各测试间标准不一的问题，本调研完成了：
1. **zlib-ng `test_data_p.h` 8 大确定性语料算法深度逆向**与 C/Swift 零分配内存生成桥接设计；
2. **极速 100% 内存矩阵基准测试引擎（50 点 < 1 秒）架构设计**，基于 Apple Silicon 硬件单调时钟与统一内存预分配池；
3. **测试套件安全瘦身与清理清单**，定位导致常规单测缓慢与环境污染的根因测试，规划完整替代与删除矩阵。

---

## R001: zlib-ng `test_data_p.h` 8 大确定性语料生成算法与 C/Swift 零分配桥接

### 1. 8 大确定性语料算法深度剖析

| 语料标识 (Corpus) | PRNG 种子 (Seed) | 核心算法与分布特征 | 目标压缩特征与流形态 |
| :--- | :--- | :--- | :--- |
| **`text`** | `0x7e47da7a` | LCG (`rng * 1103515245 + 12345`)。使用 22 个高频英文字母构建 128 词词表（长度 3~10）。采用双 7-bit 抽样取与运算 `(r1 & 127) & (r2 & 127)` 实现 **Zipf 幂律长尾分布**。1/6 概率对词尾进行变异生成孤立字面量；每 12 词注入句号换行，1/16 注入逗号，其余空格。 | 模拟真实自然语言文本：丰富的小距离短/中长匹配，夹杂标点与边界字面量。 |
| **`short_match`** | `0xc001cafe` | 维护容量为 8 的模式池（长度 3~8 字节）。4-bit 抽样状态机：<br>• `r == 0`: 长度 6~23 的 RLE 重复字节 (dist=1)；<br>• `r in 1..2`: 刷新槽位为全新随机模式；<br>• `r == 3`: 单字节随机分隔符；<br>• `r >= 4`: 重复发射池内模式（小距离短匹配链）。 | 密集短回溯引用，连续链式匹配 (Match-to-Match)，高频测试哈希桶碰撞与小窗口滑动。 |
| **`dna`** | `0x01234567` | 4 字符有限字母表 `['A', 'C', 'G', 'T']`，直接对高 8 位抽样 `bases[(rng >> 24) & 3]`。 | 3 字节前缀极高概率碰撞（仅 ^3=64$ 种组合），哈希链极长但匹配长度短，极限压测 `longest_match` 深链遍历性能。 |
| **`random`** | `0xdeadbeef` | 均匀分布伪随机字节 `(uint8_t)(rng >> 24)`。 | 纯不可压缩噪声，强制 DEFLATE 生成存储块 (Stored Blocks, BTYPE=00)，压测零拷贝直通与 INFLATE 无拷贝路径。 |
| **`literals`** | `0x600dd1ce` | 高熵字面量分布（约 6.5 bits/byte）。75% 字节采用两个均匀随机字节的按位与 `a & b`，25% 保持均匀随机以打破偶发 3 字节重复。 | 强制生成动态霍夫曼编码块 (Coded Blocks, 压缩比约 1.1x)，几乎无字典匹配，纯压测霍夫曼树构建与位流发射。 |
| **`mixed`** | `0xb1a5b1a5` | 复合二进制流状态机：<br>• 86% 代码段：字面量游程 (均值 ~6) + 4096 窗口内自复制 (长度 4~17, 距离 8~4096)；<br>• 10% 符号表：24 字节结构体重复发射，每次微变 3~4 字节；<br>• 4% 零对齐填充：64~447 字节 `memset(0)`。 | 模拟 Mach-O / ELF 目标文件与归档：字面量占比 ~80%，压缩比约 2.7x。 |
| **`realistic_rgb`** | `0x12345678` | 2D 图像网格（宽 256 或像素数）。双向对角线颜色渐变计算基色，叠加 32 级 XorShift32 扰动噪声 (`seed ^= seed<<13; seed ^= seed>>17; seed ^= seed<<5; noise = (seed & 0x1F) - 15`) 并 clamp 在 0~255。 | 模拟未压缩 24-bit 真实位图：在 dist=3 (RGB 跨度) 形成微小匹配，并在跨行形成长匹配。 |
| **`striped_rgb`** | `0` (Deterministic) | 三等分纯色 RGB 条纹（前 1/3 为纯红 `255,0,0`，中 1/3 为纯绿 `0,255,0`，后 1/3 为纯蓝 `0,0,255`）。 | 极长 dist=3 匹配以及条纹交界处的跨边界大块回溯。 |

### 2. C/Swift 零分配桥接架构方案
- **C 语言层**：`Sources/CTTZipBridge/CTTZipCorpusGen.c`，接口纯指针原地写入，0 次 malloc/free。
- **Swift 语言层**：`Sources/TTZipCore/Benchmark/BenchmarkCorpusGenerator.swift`，提供强类型枚举 `BenchmarkCorpusType` 并结合预分配页内存池复用。

### 3. R001 决策四要素
- **Decision**: 在 `Sources/CTTZipBridge/` 中原生实现 zlib-ng `test_data_p.h` 的 8 大确定性语料生成器，采用 Caller-Allocated 纯指针写入；在 `Sources/TTZipCore/Benchmark/` 封装强类型 `BenchmarkCorpusType` 与静态页面复用池。
- **Rationale**: 确定性合成语料无需任何磁盘依赖或外部 Fixture 下载，且数学上覆盖了自然文本、高碰撞短串、不可压缩噪声、2D 图像步长与 Mach-O 混合二进制等全部 DEFLATE 关键流形态；零堆分配消除了测试时的 GC/ARC 与内存碎片噪音。
- **Alternatives Considered**: 磁盘读取真实 Silesia / enwik8 文件（I/O 抖动与冷热缓存不可控）；Swift 纯层动态数组生成（额外分配开销大）。
- **Source**: `Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/test/benchmarks/data_type.cc`

---

## R002: 极速 100% 内存矩阵基准测试引擎（50 点 < 1 秒）架构设计

### 1. 50 点参数化压测矩阵架构
- `LibdeflateAccelerator`: Levels 1, 3, 6, 9, 12 × 2 尺寸 = 10 点
- `DeflateStreamEngine` / `NativeDeflate`: Levels 1, 6 × 2 尺寸 = 4 点
- `Zstd`: Levels 1, 3, 7, 19 × 2 尺寸 × 4 核心语料 = 16 点
- `LZ4`: Fast (Accel 1), Default, HC (Level 9) × 2 尺寸 × 3 语料 = 14 点
- `Snappy`: Default × 2 尺寸 × 3 语料 = 6 点
- **总耗时预算**： 	imes 8	ext{ ms} = 400	ext{ ms} = \mathbf{0.4	ext{ 秒}}$。

### 2. R002 决策四要素
- **Decision**: 统一采用 `InMemoryBenchmarkEngine` 进行 128KB/1MB 预分配页内存压测，集成 `LibdeflateAccelerator`, `DeflateStreamEngine`, `Zstd`, `LZ4`, `Snappy` 50 点参数矩阵，输出吞吐、压缩比与 CV 统计值。
- **Rationale**: 纯内存流式消除一切磁盘与文件系统锁开销；128KB/1MB 精确映射 Apple Silicon 硬件缓存层级；50 点在 0.4 秒内跑完且具备字节级 `memcmp` 断言，兼具极速与严谨。
- **Alternatives Considered**: XCTest 自带 `measure {}`（强制 10 次慢速循环无法定制预热）；磁盘临时文件压测（产生数百毫秒 I/O 延迟）。
- **Source**: `Sources/CTTZipBridge/CTTZipPlatformTimer.c`, `Sources/TTZipCore/Platform/PlatformMonotonicTimer.swift`

---

## R003: 冗余慢速测试清理清单与安全瘦身矩阵

### 1. 拟删除/归并的冗余慢速文件清单（共 11 个完全删除，5 个优化）
1. `XCTestPerformanceMeasureTests.swift` (415行) -> **完全删除**（被纯内存矩阵全面替代）
2. `SingleCoreDeflatePkTests.swift` -> **完全删除**
3. `SingleCoreDecompressPkTests.swift` -> **完全删除**
4. `BenchmarkTests.swift` -> **完全删除**
5. `ZipBenchPkTests.swift` -> **完全删除**
6. `AllFormatsPkSuiteTests.swift` -> **完全删除**
7. `CompoundMixedCorpusBenchmarkPkTests.swift` -> **完全删除**
8. `ZipSingleCoreParetoFrontierPkTests.swift` (692行) -> **完全删除**
9. `ZipMultiCoreParetoFrontierPkTests.swift` -> **完全删除**
10. `GlobalCompressionEliteParetoPkTests.swift` -> **完全删除**
11. `SevenZipParetoFrontierPkTests.swift` -> **完全删除**
12. `RealWorldPerformanceTests.swift` -> **精简归并**（保留 10 个 mock 文件的打包正确性验证，移除测速循环）
13. `DirectoryScanPerformanceTests.swift` -> **精简优化**（扫描数量从 1000 缩减至 20）
14. `SwarOptimizationBenchmarkTests.swift` -> **精简优化**（微循环从 2,000,000 降至 1,000）

### 2. R003 决策四要素
- **Decision**: 彻底删除 11 个重度冗余的 PK/Pareto 测试文件，优化 3 个慢速测试文件，将功能性字节一致性校验下沉至轻量单元测试，将性能基准测试收敛至全新的 50 点内存 Deflate-Bench 矩阵。
- **Rationale**: 单元测试的核心目标是验证功能正确性与防回归，而非在日常 `swift test` 中执行耗时数十秒的磁盘性能压测；性能与 Pareto 测算应当由具备硬件校准的独立内存基准套件在显式命令下执行。
- **Alternatives Considered**: 仅添加环境变量守卫（无法解决代码重复与维护负担）。
- **Source**: `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift`, `Tests/TTZipTests/SingleCoreDeflatePkTests.swift`
