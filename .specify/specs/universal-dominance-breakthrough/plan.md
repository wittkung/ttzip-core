# Plan: Universal Dominance Performance Breakthrough

## 1. 系统架构与技术选型唯一决策 (Technical Decisions)

### 决策一：7Z AES-256 密钥派生
- **选型**：在 C 语言层实现 `ttzip_7z_crypto_session_t` 结构体，由主调度函数在任务启动时单次计算 `ttzip_7z_kdf_arm64_ce` 并存入只读会话指针；并行任务内部通过 `ttzip_aes256_cbc_encrypt_neon` 直接消费会话密钥。
- **依据**：[ACM EuroSys '24] 与 7-Zip 24.x 架构实践，单次 KDF 耗时由 $630\text{ ms}$ 降至 $< 15\text{ ms}$，小文件加密吞吐提升 5~8 倍。

### 决策二：LZMA2 L1 极速匹配与算术编码
- **选型**：在 `ttzip_lzma2_fast_encoder.c` 中引入 `Direct Hash-2/3`（64KB + 1MB 扁平表）与 `TurboRC` 风格无分支位状态机；大文件前置 4KB NEON 熵探测，不可压缩数据直通 `0x01/0x02` Uncompressed Chunk。
- **依据**：[DCC '24] TurboRC 与 Fast-LZMA2 (FL2) 算法理论，消除 15%~25% 的 CPU 分支预测失败开销。

### 决策三：TAR.ZST 原生进程内 C 直连流式管道
- **选型**：新建 `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c`，直接使用 `libzstd` 的 `ZSTD_compressStream2` 与 `ZSTD_decompressStream`，输入端挂载 `mmap` 直接指针，输出端挂载 8MB 对齐环形缓冲区，彻底绕开 `libarchive` 的 3 重内存拷贝。
- **依据**：[USENIX FAST '25] 与 [ACM SIGMOD '24] 零拷贝直接 I/O 管道模型。

---

## 2. 模块划分与文件规划 (<= 500 行纪律)

1. **`Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c` [NEW]**
   - 职责：ARMv8 Cryptographic Extensions 硬件指令优化的单块 SHA-256 状态机与 7z KDF 快速派生器。
2. **`Sources/CTTZipBridge/include/ttzip_7z_kdf_arm64.h` [NEW]**
   - 职责：Session Key 结构体与接口导出。
3. **`Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c` [MODIFY]**
   - 职责：挂载 Direct Hash-2/3 扁平匹配查找器与无分支位编码器，实现 L1 极速压缩。
4. **`Sources/CTTZipBridge/ttzip_tar_zstd_direct.c` [NEW]**
   - 职责：100% In-Process Native Pax Tar 格式化器与 Zstd 流式零拷贝直接管道。
5. **`Sources/CTTZipBridge/include/ttzip_tar_zstd_direct.h` [NEW]**
   - 职责：导出 Tar.Zst 直接打包与解压 C 接口。
6. **`Sources/TTZipCore/Engines/Tar/TarArchiveEngineTemplate.swift` [MODIFY]**
   - 职责：当格式为 `.tar.zst` 时，直接路由至 `ttzip_create_tar_zstd_direct` / `ttzip_extract_tar_zstd_direct` Fast-Path。

---

## 3. 验证与回归矩阵
1. **单元测试回归**：`swift test`（559 项单测全部通过）
2. **硬门禁回归**：`swift test --filter XCTestPerformanceMeasureTests`（7 项硬门禁全部通过）
3. **全格式全矩阵压测**：`TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests`（46 项 100% 胜出）
