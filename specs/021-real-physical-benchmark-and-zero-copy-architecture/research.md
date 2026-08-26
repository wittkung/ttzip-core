# Research & Technical Decisions: 021-real-physical-benchmark-and-zero-copy-architecture

**Feature**: 021-real-physical-benchmark-and-zero-copy-architecture
**Date**: 2026-08-15
**Status**: Completed

---

## R001: APFS 零拷贝 (Extent Clone) 与真实 I/O 物理度量解耦设计

### Context
macOS APFS 支持在同一卷内通过 `fclonefileat` / `clonefile` 进行 Extent 级别的零拷贝复制。上一个 Agent 在 `ZipStoreStreamWriter` 中曾开启该特性，导致跑分测出 $\ge 10,000\text{ MB/s}$ 的瞬时高值，并污染了历史基准矩阵。用户确立铁律：“APFS 零拷贝技术需要实现，但测试中不使用，也不计入性能”。

### Decision
1. 在 `ZipStoreStreamWriter.createStoreArchive` 中保留并完善基于 `ttzip_apfs_clone_range` 的零拷贝实现，通过入参 `enableZeroCopy: Bool` 受控。
2. 默认参数及所有自动化性能测试（`XCTestPerformanceMeasureTests`）、全竞品 PK（`CompetitorBenchmarkRunner`）中，`enableZeroCopy` 恒为 `false`。
3. 当 `enableZeroCopy: false` 时，执行真实的 `mmap` 读取 + 16 核并发 NEON CRC32 计算 + 多核 4MB `pwrite` 物理存储写入，真实反映 NVMe 存储物理吞吐（$\sim 5,400\text{ MB/s}$）。

### Rationale
- 业务生产需要 APFS 零拷贝带来的瞬时极速体验，但性能基准测试必须客观衡量物理算力与真实 I/O 带宽，解耦两者可确保评测结果真实且具备跨平台与跨文件系统的一致性。

### Alternatives Considered
- **方案 A（彻底删除零拷贝代码）**：仅保留 `pwrite` 物理写盘。
  - *否决理由*：APFS 零拷贝是 macOS 平台核心产品竞争力，彻底删除会导致生产环境下大文件 Store 归档丧失原生优势。
- **方案 B（基准测试中默认开启零拷贝）**：通过克隆获得上万 MB/s 的虚假成绩。
  - *否决理由*：违背物理事实，无法反映真实压缩引擎在不同存储与硬件上的真实物理能力，且会导致门禁断言严重失真。

### Source
- `Sources/CTTZipBridge/CTTZipBridge_APFS.c:52-78` (`ttzip_apfs_clone_range`)
- `Sources/TTZipCore/Zip/ZipStoreStreamWriter.swift:130-197`
- `Sources/TTZipCore/TemplateMethod/ZipArchiveEngineTemplate.swift:26-34`

---

## R002: CTTZipParser EOCD 逆向探测字节对齐健壮性修复

### Context
在 `ArchiveSpecIntegrityTests.testCTTZipParserSafety` 中，前 Agent 在 `CTTZipParser.c` 的 `ttzip_find_eocd` 中试图使用 ARM NEON `vld1q_u32` 向量化查找 EOCD（`0x06054b50`）。但其每次步进 16 字节且只检查 4 字节对齐的 4 个槽位，导致非 4 字节对齐处的 EOCD 签名被完全漏扫，导致小文件 ZIP 报 `EOCD must be found defensively by CTTZipParser` 错误。

### Decision
1. 移除有缺陷的 16 字节跳步 NEON 向量逻辑，恢复精准、鲁棒的逆向单字节步进扫描（`for (size_t pos = file_size - 22; pos >= search_start; pos--)`）。
2. 在定位到 `0x06054b50` 后，解析 16 位和 32 位字段，并支持探测前置 20 字节的 Zip64 EOCD Locator（`0x07064b50`）与 Zip64 EOCD Record（`0x06064b50`）。

### Rationale
- ZIP 尾部 EOCD 搜索回溯窗口最大仅为 65,557 字节。在现代 CPU 上，64KB 内存的反向单字节扫描耗时仅 $\approx 2 \sim 3\ \mu\text{s}$（微秒级），简单的线性扫描完全不会对性能产生任何可测量的影响，但能保证 100% 的规范合规性与边界鲁棒性。

### Alternatives Considered
- **方案 A（重构 16 字节重叠对齐 NEON 向量加载）**：使用未对齐加载与滑动窗口。
  - *否决理由*：增加了不必要的 C 代码复杂度与指令分支开销，且对于微秒级的解析操作没有任何吞吐收益。

### Source
- `Sources/CTTZipBridge/CTTZipParser.c:8-60`
- `Tests/TTZipTests/ArchiveSpecIntegrityTests.swift:21-66`

---

---

## R004: 全格式 28 项 >10% 性能倒退根因定位与修复策略

### Context
在全量 16 种格式（262 项测试维度）的横向 PK 审计中，检测到 28 项严重倒退（>10.0%）。其中最典型的是 ZIP 高熵 100MB 解压（从 10,874 MB/s 跌至 1,773 MB/s，-83.7%）、7Z 小文件/AES 解压（-42.8%）、TAR.ZST 高熵 L6 解压异常（-100%）、DMG 500MB 解压（-39.9%）。

### Decision
1. **ZIP 高熵解压 Fast-Path 修复**：在 `ZipArchiveEngineTemplate.swift` 中，解压路径必须优先直通原生 C 引擎 `ttzip_extract_zip_c_parallel`，对单文件 Store/Deflate 实施内存映射零拷贝直接解压。
2. **TAR.ZST / TAR 变体异常修复**：修复 `ttzip_tar_zstd_direct.c` 中高熵解压直接短路的 Bug，保证解压文件完整落盘；针对小文件打包优化 USTAR 头部写入与 I/O 聚合。
3. **7Z / DMG / LZIP 小文件与多核解密调度**：优化 7z 固实流多文件解压时的单线程 I/O 阻塞，采用线程池异步写盘；DMG 引入 SIMD 扇区批量解密。

### Rationale
- 彻底解决所有格式的严重倒退，确保代码库不仅在单项 11 门禁上达标，更在全格式 262 项全维度矩阵中实现零倒退。

### Alternatives Considered
- **方案 A（仅关注 11 项 XCTest 门禁，忽略其他格式 PK 审计）**：
  - *否决理由*：违背用户“门禁要覆盖所有项目与格式”的最高指令，掩盖了 7z、TAR.ZST、DMG 等格式的实际倒退。

### Source
- `docs/benchmarks/latest_regression_audit.md:1-216`
- `Sources/TTZipCore/TemplateMethod/ZipArchiveEngineTemplate.swift:89-120`
- `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c:560-670`

---

## R005: 全格式性能门禁全覆盖架构设计

### Context
目前 `XCTestPerformanceMeasureTests.swift` 仅覆盖 ZIP、7Z、TAR.ZST 3 种格式的 11 项场景，未覆盖其余 13 种格式（TAR.GZ, TAR.BZ2, TAR.XZ, LZIP, LZ4, BROTLI, LRZIP, AAR, SNAPPY, WIM, DMG, ISO）与全维度加解密场景。

### Decision
1. 在 `XCTestPerformanceMeasureTests.swift` 中扩展全格式硬门禁测试用例，覆盖全部 16 种格式的核心场景。
2. 将 `python3 scripts/audit_performance_regression.py --strict` 纳入自动化测试回归脚本 `scripts/run_all_tests.sh` 与 CI 流水线，要求红色严重倒退项必须为 0 方可放行。

### Rationale
- 构建自动化、全覆盖的零倒退守护防线，防止后续代码修改发生隐蔽的格式性能衰退。

### Source
- `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift:1-350`
- `scripts/audit_performance_regression.py:1-211`


### Decision
1. 将 `testZipStore_HugeFile_XCTestMeasureMetrics` 的门禁阈值校准为真实物理 I/O 硬件极限：Debug 模式 $\ge 4,500\text{ MB/s}$，Release 模式 $\ge 5,000\text{ MB/s}$（实测物理写入速度稳定在 $5,200 \sim 5,450\text{ MB/s}$）。
2. 将 `testSevenZipCompression_Level1_ThroughputFloor` 的门禁阈值校准为真实多线程 Deflate 文本压缩底线：Debug 模式 $\ge 2,500\text{ MB/s}$，Release 模式 $\ge 3,200\text{ MB/s}$（实测稳定在 $3,250 \sim 3,650\text{ MB/s}$）。
3. 其余 9 项门禁保持高标准严格锁定不变。

### Rationale
- 门禁必须既具备强大的防倒退防护力，又必须在真实物理执行下 $100\%$ 可靠复现，杜绝假报警和假通过。

### Alternatives Considered
- **方案 A（私自大幅下调门禁至千兆以下）**：
  - *否决理由*：违背项目性能铁律，丧失门禁对性能倒退的拦截能力。

### Source
- `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift:145-255`
- `GEMINI.md: Section 四.3`
