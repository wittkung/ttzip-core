# Research: 023-last-mile-zero-regression-and-adaptive-peak-gates

## 1. WIM 500MB 大文件解压吞吐稳定跨越 11,000+ MB/s

- **Decision**: 在 `Sources/CTTZipBridge/ttzip_native_archive.c` 中显式收录 `.wim` 探测，并在解压前下发 `fcntl(fd, F_RDAHEAD, 1)` 与 `posix_madvise(MADV_WILLNEED | MADV_SEQUENTIAL)` 预热提示，写盘缓冲区按 Apple Silicon 16KB 原生页边界对齐。
- **Rationale**: 连续压测尾声由于多次 500MB 大文件写盘，APFS 脏页排队触发系统后台刷盘，缺少预取与 16KB 页边界对齐会导致冷页缺页中断与跨页写 RMW 惩罚；预加载和页对齐直通使得解压稳定在 Unified Memory 极速通道（$\ge 11,000\text{ MB/s}$）。
- **Alternatives Considered**:
  - *使用 `fcntl(fd, F_NOCACHE, 1)`*: 强制同步刷入 NVMe，受限于物理 SSD 写入瓶颈（~3,500 MB/s），无法达到 $\ge 11,000\text{ MB/s}$ 门禁。
  - *自研脱离 libarchive 的 WIM 解析器*: 维护成本过高且破坏格式兼容性与安全性。
- **Source**: `Sources/CTTZipBridge/ttzip_native_archive.c:65-76, 202-216`, `Sources/CTTZipBridge/ttzip_tar_native.c:227-260`, `specs/023-last-mile-zero-regression-and-adaptive-peak-gates/spec.md:25-30`.

---

## 2. DMG 10MB 拟真日志与 100MB 高熵镜像解压消除调度抖动

- **Decision**: 采用 100% 进程内纯 C 原生流式解压引擎，并在 `ArchiveExtractor+Dispatch.swift` 与 `TarArchiveEngineTemplate.swift` 中为 `.dmg` 建立独立直通 Fast-Path，严格隔离 7Z 引擎试探性 Header 校验。
- **Rationale**: DMG 格式不具备 7Z 签名，先前共用分发分支触发了无效的 `open + mmap + parse header fail + munmap + close` 试错开销（产生 ~0.3ms - 0.5ms 延迟抖动并破坏 CPU L1/L2 缓存）；直通分发使拟真日志 L6 解压达 $\ge 6,562.6\text{ MB/s}$，高熵 Payload 达 $\ge 9,556.6\text{ MB/s}$。
- **Alternatives Considered**:
  - *调用 macOS `hdiutil attach` + `/bin/cp` 挂载拷贝*: 产生 50ms~200ms 进程拉起开销与 `diskimagesiod` IPC 阻塞，吞吐跌破 80% 且无法在 MAS 沙盒运行。
  - *调用外部 `7zz` CLI*: 违背零外部进程调用铁律。
- **Source**: `Sources/TTZipCore/ArchiveExtractor+Dispatch.swift:16-21`, `Sources/CTTZipBridge/ttzip_native_archive.c:202-216`, `docs/benchmarks/benchmark_report_2026-08-15_071939.json:2207-2227`.

---

## 3. 7Z 海量小文件 (100文件) 解压栈上双层零分配内联目录缓存池

- **Decision**: 在 `Sources/CTTZipBridge/CTTZipBridge_7zNativeDecoder.c` 的写盘热循环中实现**栈上双层零分配内联目录缓存池 (L1 Single-Slot Hot Cache + L2 64-Slot Hash Table)**。
- **Rationale**: 原逻辑对 100 个同目录文件重复触发 $100 \times 6 = 600$ 次 `mkdir()` 逐级系统调用，产生 ~3ms 纯内核态锁争抢；双层缓存池将系统调用压降 $>98\%$（降至 1 次），零堆分配、零跨线程锁，将 100 小文件解压吞吐从 1,185.9 MB/s 恢复至 $1,450+\text{ MB/s}$ 历史峰值。
- **Alternatives Considered**:
  - *在 Swift 层预建目录树*: 需要将 C 结构转换为 Swift 字符串数组并做堆分配，破坏零抽象并增加 FFI 桥接开销。
  - *在全局 `ttzip_common_mkdir_p` 增加全局静态缓存*: 必须引入互斥锁，引发并发线程锁争抢。
- **Source**: `Sources/CTTZipBridge/CTTZipBridge_7zNativeDecoder.c:150-209`, `Sources/CTTZipBridge/CTTZipCommon.c:37-64`, `Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner+Setup.swift:35-38`.

---

## 4. 全格式历史最优硬性能门禁全覆盖与动态校准

- **Decision**: 在 `scripts/audit_performance_regression.py` 与 `GEMINI.md` 中将全格式 16 种格式所有 262 项维度的门禁严格绑定至历史最高峰值，并在每次刷新记录时自动更新 `docs/benchmarks/peak_performance_matrix.json`。
- **Rationale**: 彻底杜绝下调门禁或掩耳盗铃，确保性能优化只增不减，任何格式出现 $>10\%$ 倒退直接阻断流水线。
- **Alternatives Considered**:
  - *仅监控主要格式 (ZIP/7Z)*: 容易造成冷门格式（如 WIM/DMG/LZIP）的性能暗中劣化。
- **Source**: `GEMINI.md:124-148`, `scripts/audit_performance_regression.py:35-48`.
