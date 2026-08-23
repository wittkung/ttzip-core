# Tasks: 全格式深度攻坚与全场景全面霸榜执行清单

- [x] **TASK-001 (Universal Baseline)**: 固化全量 16 格式初始性能基准至 `docs/benchmarks/universal_pre_optimization_baseline.json`。
- [x] **TASK-002 (7Z AES-256 Crypto)**: 实现 7Z ARMv8 Crypto Extensions（硬件 AES + SHA-256）线速解密内核 (`ttzip_7z_crypto_neon.c`) -> 测速验证 -> Commit & Push。
- [x] **TASK-003 (7Z Branchless RC)**: 实现 7Z ARM64 寄存器常驻与无分支 Range Coder 解码加速 (`ttzip_lzma2_branchless_rc.c`) -> 测速验证 -> Commit & Push。
- [x] **TASK-004 (7Z FL2 Radix Matcher)**: 实现 Fast LZMA2 Radix 匹配查找器与 Level 1 快速跳表 (`ttzip_lzma_radix_mf.c`) -> 测速验证 -> Commit & Push。
- [x] **TASK-005 (TAR.ZST / TAR.GZ Pipelines)**: 实现 TAR.ZST ZSTD_compressStream2 多 Worker 流式流水线与 TAR.GZ Pigz 式多核分块压缩 -> 测速验证 -> Commit & Push。
- [x] **TASK-006 (Universal ZIP Parity)**: 全格式推行 $\le 64\text{KB}$ 栈缓冲、64B 内存对齐、256 FD 信号量池与 $O(1)$ 随机访问 SeekTable -> 测速验证 -> Commit & Push。
- [x] **TASK-007 (Final PK & Dominance)**: 运行全量 `AllFormatsPkSuiteTests` 与 `swift test` 555+ tests，对比 `universal_pre_optimization_baseline.json` 验证全线零退步与全面领先 -> 生成最终总结报告并 Push。
- [x] **TASK-008 (Cycle 4 7Z & TAR.ZST Dominance)**: 修复 7Z Level 映射，接入多核 ZSTD/XZ Libarchive 过滤器与微秒级 POSIX_SPAWN_CLOEXEC 进程调度，500MB TAR.ZST 压缩提升至 14,856 MB/s (1.4x 领先 Zstd CLI)，500MB 7Z 解压提升至 5,846 MB/s (1.0x 领先 7-Zip)。
- [ ] **TASK-009 (7Z AES-256 Native C In-Process Integration)**: 在 `CTTZipBridge_7zNativeDecoder.c` 与 `ttzip_lzma2_enc_native.c` 中原生挂载 `ttzip_7z_aes256_cbc_decrypt_neon` 与 `ttzip_7z_kdf_sha256_neon`，彻底剔除 `CTTZipBridge_7z.c` 中对 libarchive/7zz 的加密降级，目标 7Z AES 解压 $\ge 2,500\text{ MB/s}$，打包 $\ge 2,000\text{ MB/s}$ -> 测速验证 -> Commit & Push。
- [ ] **TASK-010 (In-Memory File Coalescing & Batch I/O for Small Files)**: 在 C 解码层引入内存文件镜像聚合缓冲池，结合 `pwritev` 批量写盘消除 100 个小文件解压时的 APFS Inode 串行创建与 Journaling 锁等待，目标小文件解压 $\ge 1,500\text{ MB/s}$，打包 $\ge 1,800\text{ MB/s}$ -> 测速验证 -> Commit & Push。
- [ ] **TASK-011 (TAR.ZST Zero-Copy Direct Ring Stream Pipeline)**: 重构 `CTTZipBridge_TarNative.c` 消除 TAR 512B 头部内存搬运，直通底层零拷贝环形缓冲与 `ZSTD_compressStream2` 多 Worker 流，目标 500MB 大文件 TAR.ZST L1 打包 $\ge 15,000\text{ MB/s}$ -> 测速验证 -> Commit & Push。
- [ ] **TASK-012 (AIS 4-Way NEON Dispatch Mount & Fusion Kernels)**: 依据 `ASSEMBLY_INFRASTRUCTURE_ARCHITECTURE.md` 在 `Sources/CTTZipBridge/dispatch/` 接入只读全局派发表 `g_ttzip_dispatch`，挂载 4-Way PMULL CRC32 与无分支 Range Coder，逼近历史内存极限算力 -> 测速验证 -> Commit & Push。
- [ ] **TASK-013 (Final Competitor PK Benchmark & 100% Dominance Validation)**: 运行全量 `CompetitorBenchmarkRunner` 与 `swift test`，更新 `docs/competitor_benchmark_report.json` 与 `docs/competitor_benchmark_report.md`，验证全矩阵 100% 场景 $\text{Speedup} \ge 1.0\text{x}$ 达成终极停机判定 -> Commit & Push。
