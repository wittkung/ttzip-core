# Quickstart Guide: 全覆盖测试与基准遥测零回退验证 (Feature 162)

## Scenario 1: 执行全算法 C11 微架构基准测试
- **Command**:
  ```bash
  cmake --build build --target ttzip_benchmark_runner && ./build/ttzip_benchmark_runner --codecs --all
  ```
- **Expected Output**:
  - 输出 10 大算法（Deflate L1/6/9, Zstd L1/3, Fast-LZMA2 L3, LZ4 L1/9, LZFSE, Snappy, Brotli, Bzip2, Blosc2）在标准语料下的双向吞吐与 CPB；
  - CRC32 吞吐 >= 65 GB/s (CPB <= 0.050)；
  - 100% 内存往返校验通过。
- **Failure Diagnostic**:
  - 若 CPB 异常变大，检查是否引入了动态 `malloc` 或多余的函数指针间接跳转；
  - 若 `memcmp` 校验失败，检查特定压缩级别的缓冲区边界计算。

---

## Scenario 2: 执行全格式容器解压基准测试
- **Command**:
  ```bash
  cmake --build build --target ttzip_benchmark_runner && ./build/ttzip_benchmark_runner --formats
  ```
- **Expected Output**:
  - 针对 ZIP (Store/Deflate), TAR.GZ, TAR.ZST, TAR.BZ2, TAR.XZ, 7Z, UnRAR 跑出打包和解包的 MB/s 与挂钟时间；
  - 记录 `Peak RSS <= 64 MB`，100% 文件校验匹配。
- **Failure Diagnostic**:
  - 若解压失败，检查 `/tmp/` 下临时目录创建权限或解密密钥参数。

---

## Scenario 3: 运行 5 重物理闸门零回退门禁
- **Command**:
  ```bash
  ./scripts/run_optimization_gate.sh --bail --json build/gate_report.json
  ```
- **Expected Output**:
  - `[Gate 1/5] Native C11 Microkernel & Unit Test Suites ... [PASS]`
  - `[Gate 2/5] C Microarchitectural PMU, Checksum & Codec Benchmark ... [PASS]`
  - `[Gate 3/5] 50-Point Matrix Stability & CV Gate ... [PASS]`
  - `[Gate 4/5] 160-Point Compression Delta Engine & Binary Size Audit ... [PASS]`
  - `[Gate 5/5] End-to-End CLI I/O and Process Peak RSS Gate ... [PASS]`
  - `Overall Verdict: PASS`
- **Failure Diagnostic**:
  - 若 Gate 4 失败，查看 `build/delta_report.json` 中哪些级别出现了 `REGRESSION`；
  - 若 Gate 3 失败，排查是否有高变异系数（$CV\% > 1.5\%$）的算法受到 CPU 降频或后台抢占干扰。
