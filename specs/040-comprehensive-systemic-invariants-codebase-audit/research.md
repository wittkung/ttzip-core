# Phase 0 Research: 全仓库系统级不变量深度审计综合报告

**Feature Directory**: `specs/040-comprehensive-systemic-invariants-codebase-audit`  
**Date**: 2026-08-16  
**Status**: Completed  
**Sources Baseline**: `Sources/CTTZipBridge/` (46 files), `Sources/TTZipCore/` (50+ files), `Sources/TTZipApp/`, `Sources/TTZipCLI/`, `Tests/` (525+ tests)

---

## 缺陷统计全景概览

| 审计维度 | 扫描文件数 | P0 (致命漏洞/崩溃) | P1 (严重隐患/OOM) | P2 (性能衰减/微元缺陷) | P3 (低危/代码异味) |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **Layer 1: C 桥接层 (`Sources/CTTZipBridge/`)** | 46 | 5 | 7 | 3 | 0 |
| **Layer 2: Swift 核心引擎 (`Sources/TTZipCore/`)** | 52 | 5 | 6 | 5 | 3 |
| **Layer 3 & Tests: UI/CLI 与测试套件 (`Tests/`, `App`)** | 80+ | 1 | 3 | 2 | 1 |
| **全库合计** | **178+** | **11** | **16** | **10** | **4** |

---

## 一、 R001: C 桥接层与底层引擎系统级不变量审查成果 (Subagent 1)

### 1. 核心缺陷清单与源码定位
1. **Stream-First 维度**:
   - `CTTZipBridge_7zSolid.c:56-61, 123-130`: 一次性全量 `posix_memalign` 分配全部待打包文件大小（20GB 产生 42GB RAM 请求，触发 OOM 崩溃）[P0]。
   - `ttzip_lzma2_enc_native.c:201-207, 431-437`: 非零拷贝与 AES 加密全量 `malloc` 堆缓冲区，内存翻倍 [P0]。
   - `ttzip_7z_block_decoder.c:103`: 一次性 `posix_memalign` 解压全量数据 [P1]。
   - `CTTZipExtract.c:300-303`: 单大文件单次堆分配 `malloc(e->uncompressed_size)` [P1]。
   - `CTTZipBridge_LZFSE.c:82-84, 128-130`: 按压缩包大小的 8 倍做过度预分配 [P1]。
2. **Invariant-First 维度**:
   - `CTTZipBridge_7z.c:404` 与 `ttzip_tar_native.c:265`: 遗漏 `ARCHIVE_EXTRACT_SECURE_SYMLINKS` 与 `ARCHIVE_EXTRACT_SECURE_NOABSOLUTEPATHS`（严重 Zip Slip 漏洞）[P0]。
   - `CTTZipBridge_7zNativeDecoder.c:169`: 使用 `snprintf` 拼接路径而未做 `ttzip_common_join_path` 校验 [P0]。
   - `CTTZipBridge_Archive.c:215` 等 4 处: 忽略 `ttzip_common_join_path` 的错误返回值 [P1]。
   - `CTTZipBridge_7zNativeDecoder.c:158` 等 5 处: 固定栈缓冲区（1024/512 字节）在深层目录发生截断，破坏目录树 [P1]。
3. **Bounds-First 维度**:
   - `ttzip_io_file_list_t`、`parallel_gz_ctx`、`ttzip_stream_coder_t` 等 8 大自定义 C 结构体缺少 Magic 首字段与 `magic = 0` 析构清零 [P1]。
   - `ttzip_lzma2_enc_native.c:121`、`ttzip_7z_kdf_arm64.c:181`、`CTTZipBridge_7zNativeDecoder.c:87` 等 6 处: 密码与派生 AES 密钥使用普通 `memset` 或未擦除，被 Clang 死存储优化（DSE）剔除 [P0]。
   - `CTTZipBridge_GzParallel.c:247` 等 6 处: 64 位整数向 `size_t`/`ssize_t` 强转缺少 `SSIZE_MAX` Clamp [P1]。

---

## 二、 R002: Swift 核心管道与设计模式数据平面合规审查 (Subagent 2)

### 1. 核心缺陷清单与源码定位
1. **Stream-First 维度**:
   - `ZipBlockParallelCompressor.swift:21, 45, 60-66`: 循环中使用 `Data(count:)` 产生内核物理页清零中断，并在内存中多次拼接聚合 [P2]。
   - `ZipParallelWriter.swift:27, 32, 89, 112, 132-190`: `compressedResultsBox` 收集全量压缩后 Payload，50GB 数据集产生数十 GB 瞬时堆常驻导致 OOM [P1]。
   - `ArchivePipelineProducerConsumerEngine.swift:38-42, 124`: 幽灵享元借还（从池中借出清零但实际执行 `handle.read` 重新堆分配）[P2]。
   - `ArchiveReader.swift:135-179` & `PasswordRecoveryEngine.swift:147-172`: 目录探索与字典爆破尝试每次都全量解压写盘，造成 SSD 极端磨损 [P1]。
2. **Invariant-First 维度**:
   - `ZipParallelExtractor.swift:56-71, 85-96` & `ZipCentralDirectoryReader.swift:146-153`: `sanitizePath` 未拦截 `../../`，`open` 未携带 `O_NOFOLLOW`，存在致命 Zip Slip 与软链接逃逸漏洞 [P0]。
   - `SecurityScanner.swift:20-30`: Windows 风格反斜杠 `\` 未标准化，可被绕过 [P1]。
   - `SevenZipCAdapter.swift:68-81` & `ArchiveReader.swift:144-152`: 降级调用外部 `Process("p7z -p\(pwd)")` 进程，命令行暴露明文密码 (CWE-214) [P0]。
3. **Bounds-First 维度**:
   - `SevenZipCryptoEngine.swift:81-88`: 并发 CBC 加密未更新 `chunkIV`，导致密文损坏无法解密 [P0]。
   - `MemoryPageFlyweightPool.swift:46, 105, 121`: `NSLock` 热路径全局竞争，每次借出执行冗余 `memset`，分配失败强制解包崩溃 [P1]。
   - `CUnsafeBufferAdapter.swift:20-81`: 5,000 条目时尾递归调用导致 5,000 层栈深度，触发栈溢出崩溃 [P1]。
   - `ZipCentralDirectoryReader.swift:88-121`: 算术运算缺少溢出保护，Zip64 负数越界寻址 [P0]。
   - `SevenZipBlockParallelDecompressor.swift:41-53`: 解压后直接调用 `free(dstRawPtr)`，解压产物被幽灵丢弃 [P0]。
   - `BaseArchiveEngineTemplate.swift:130-141`: 协程中使用 `DispatchSemaphore.wait()` 导致线程池饥饿死锁 [P1]。

---

## 三、 R003: 测试套件预言机有效性与应用层防御审计 (Subagent 3)

### 1. 核心缺陷清单与源码定位
1. **Oracle-First 维度**:
   - `ArchiveMutationFuzzTests.swift:42-83`: 变异后的二进制 ZIP 数据被错误传入 `UUDecoder.decode`，首行失败直接返回 `nil`，导致 100 次模糊测试虚假通过，引擎解码器零覆盖 [P0 逻辑失效]。
   - `ArchiveGoldenCorpusTests.swift:57-70`: 仅验证了 UUDecode 文本解码吞吐，未将还原后的数据传入引擎解压并断言结构 [P1 语料未接入]。
   - 12 种归档格式在 `GoldenCorpus/` 中语料为 0 [P1]。
   - `FormatSupportTests.swift`、`ArchiveWriterTests.swift` 等测试套件 100% 存在“自产自销”同义反复模式，缺乏系统原生工具（`/usr/bin/tar`、`/usr/bin/unzip`）双向差分测试 [P1]。
2. **分层架构与 UI 安全维度**:
   - `PasswordVaultPopoverView.swift:39`、`ExtractModalView.swift:124` 等 6 处: 在 Popover/Sheet 中使用 `SecureField`，违反规则触发 macOS TSM 中文输入法全局死锁 [P1]。
   - `TTZipApp.swift:4` & `TTZipCLIApp.swift:3`: UI 与 CLI 越级 `import CTTZipBridge`，破坏分层单向依赖 [P2]。

---

## 四、 综合重构决议 (Consolidated Decisions)

1. **安全与正确性阻断修复 (Phase A / P0)**：
   - 补齐 C 桥接与 Swift 核心解压器中的全量 Zip Slip 与 `O_NOFOLLOW` 符号链接防御。
   - 修复 CBC 并发加密 IV 逻辑、7z 块解压丢弃与 `Process()` 明文密码泄漏。
   - 修复 `ArchiveMutationFuzzTests` 模糊测试逻辑错位问题，使其真实注入解压引擎。
   - 替换所有 Popover/Sheet 中的 `SecureField` 消除中文输入法阻塞。
2. **流式管道与内存控制 (Phase B / P1)**：
   - 7z 固实压缩重构为 64MB 滑动窗口分块流式管道；`ZipParallelWriter` 改用预计算 Offset 直写。
   - 消除全量解包探查与字典爆破 SSD 磨损，改为纯内存 Header 验证。
   - 彻底将敏感内存擦除升级为 `memset_s`。
3. **性能微元与测试预言机强化 (Phase C / P2)**：
   - 消除热循环 `Data(count:)`，优化享元池与递归栈分配。
   - 全格式补齐系统原生 CLI 双向差分测试套件。
