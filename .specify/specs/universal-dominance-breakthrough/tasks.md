# Tasks: Universal Dominance Performance Breakthrough

- [ ] **TASK-1: 7Z AES-256 会话级密钥缓存与 ARMv8 硬件 Crypto 加速**
  - [ ] 1.1 创建 `Sources/CTTZipBridge/include/ttzip_7z_kdf_arm64.h` 与 `ttzip_7z_kdf_arm64.c`，使用 ARM64 Crypto Extensions 优化 $2^{19}$ 轮 SHA-256 迭代循环。
  - [ ] 1.2 在 `ttzip_lzma2_enc_native.c` 与 `ttzip_create_7z_lzma2_native_c` 中实现 `ttzip_7z_crypto_session_t` 会话级密钥派生与只读共享，消除多文件重复 KDF 计算。
  - [ ] 1.3 验证海量小文件 7Z L1 AES-256 打包吞吐达到 $\ge 1500\text{ MB/s}$（从 0.57x 提升至 $\ge 2.0\text{x}$ 领先）。

- [ ] **TASK-2: Fast LZMA2 L1 Direct Hash 匹配器与无分支 Range Coder**
  - [ ] 2.1 在 `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c` 中实现 $O(1)$ 复杂度的 `Direct Hash-2/3` 扁平匹配查找器。
  - [ ] 2.2 引入 `csel` / 位掩码加速的无分支 Range Coder 状态机，消除 15%~25% 分支预测失败惩罚。
  - [ ] 2.3 验证 500MB 大文件 7Z L1 与拟真日志 L1 打包吞吐达到 $\ge 6000\text{ MB/s}$（从 0.76x / 0.84x 提升至 $\ge 1.3\text{x} \sim 1.6\text{x}$ 领先）。

- [ ] **TASK-3: TAR.ZST 100% In-Process Native Direct mmap 管道**
  - [ ] 3.1 创建 `Sources/CTTZipBridge/include/ttzip_tar_zstd_direct.h` 与 `ttzip_tar_zstd_direct.c`，实现零拷贝 Pax Tar 512B 写入器与 `ZSTD_compressStream2` / `ZSTD_decompressStream` 直接流式对接。
  - [ ] 3.2 针对 Apple Silicon 物理拓扑配置 `jobSize = 8MB`，`overlapLog = 3` 与 8MB 页面对齐环形缓冲。
  - [ ] 3.3 在 `TarArchiveEngineTemplate.swift` 中将 `.tar.zst` 路由至 Native Direct Fast-Path。
  - [ ] 3.4 验证 500MB 大文件 TAR.ZST L1 打包吞吐突破 $18,000\text{ MB/s}$（从 0.82x 提升至 $\ge 1.2\text{x}$ 领先），解压突破 $8,000\text{ MB/s}$。

- [ ] **TASK-4: 全格式全场景自动化回归与硬门禁大闭环**
  - [ ] 4.1 运行 `swift test` 确保 559 项单测 100% PASS。
  - [ ] 4.2 运行 `swift test --filter XCTestPerformanceMeasureTests` 确保 7 项硬门禁全部达标。
  - [ ] 4.3 运行 `TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests`，生成最新基准报告，确保 46 组场景 100% 胜出。
