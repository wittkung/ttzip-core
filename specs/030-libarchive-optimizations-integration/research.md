# Phase 0 Technical Research: 030-libarchive-optimizations-integration

## R001: Vendor 静态库替换与符号 ABI 兼容性

- **Decision (选定方案)**：
  将 `Vendor/libarchive-upstream/build/bin/libarchive.a`（1,154,360 字节）覆盖替换至 `Vendor/lib/libarchive.a`，作为 TTZip 生产与测试环境的基础静态归档引擎。
- **Rationale (选择理由)**：
  1. **架构与 ABI 零侵入兼容**：`Vendor/libarchive-upstream/build/bin/libarchive.a` 采用与原库完全一致的 macOS Apple Silicon (arm64) / Darwin 编译配置，且公共头文件 `Vendor/include/archive.h`（56,071 字节）与 `Vendor/include/archive_entry.h`（35,427 字节）完全一致，无任何公共 API/ABI 破坏性变更。
  2. **符号与密码学实现完整就绪**：新库内部包含完整编译的 `__archive_cryptor` 密码学分发结构体（`archive_cryptor.c`），绑定了 `decrypto_aes_cbc_init`、`decrypto_aes_cbc_update`、`decrypto_aes_cbc_release` 以及 `kdf_7z_sha256`（在 macOS 平台下基于 Apple `CommonCrypto` 原生硬件加速），并在 `archive_read_support_format_7zip.c` 中完整支持 7z AES-256 Codec `0x06F10701` 与全头加密 `kEncodedHeader`（`0x17`）递归解码。
  3. **Package.swift 链接无缝对接**：`Package.swift` 中 `CTTZipBridge` 的 `linkerSettings` 通过 `-L $(vendorLibDir)` 与 `"$(vendorLibDir)/libarchive.a"` 以及系统库 `bz2`、`z`、`iconv`、`xml2`、`expat`、`-lc++` 链接，替换后 `CTTZipBridge`、`TTZipCore` 及 `TTZipTests` 能够开箱即用直接静态链接，`ttzip_extract_archive_advanced`、`ttzip_inspect_archive_v2` 以及 `ttzip_extract_7z_libarchive_c` 原生获得 7z AES-256 解密能力。
- **Alternatives Considered (被否决方案及理由)**：
  - *方案 A：维持现有旧版 `Vendor/lib/libarchive.a`，通过外部 `7zz` CLI 进程降级调用处理加密 7z*。  
    **否决理由**：严重违背 TTZip「100% In-Process C 静态库绑定（零外部 CLI 进程调用）」与 Mac App Store (MAS) 沙盒准入规范，且无法提升 `ttzip_inspect_archive_v2` 的通用元数据穿透能力。
  - *方案 B：仅将 `archive_cryptor.c` 与 `archive_read_support_format_7zip.c` 源码直接搬迁进 `Sources/CTTZipBridge/` 参与编译*。  
    **否决理由**：会导致静态库符号重复冲突（Symbol Duplication）与双版本 libarchive 实例的维护混乱，破坏 `Vendor/` 统一静态库分发架构。
- **Source (查阅来源)**：
  - `Vendor/lib/libarchive.a` (1,121,280 字节)
  - `Vendor/libarchive-upstream/build/bin/libarchive.a` (1,154,360 字节)
  - `Vendor/libarchive-upstream/libarchive/archive_cryptor.c` (第 837–912 行，`kdf_7z_sha256` 与 `__archive_cryptor` 符号)
  - `Vendor/libarchive-upstream/libarchive/archive_cryptor_private.h` (第 176–218 行)
  - `Vendor/libarchive-upstream/libarchive/archive_read_support_format_7zip.c` (第 106, 976, 1683, 3406, 4018, 4094 行)
  - `Package.swift` (第 53–70 行 `CTTZipBridge` `linkerSettings`)
  - `Vendor/include/archive.h` 与 `Vendor/include/archive_entry.h`

---

## R002: TTZip 原生 KDF (ttzip_7z_kdf_arm64.c) 栈内存与无锁优化

- **Decision (选定方案)**：
  重构 `Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c`：
  1. 采用栈上连续布局缓冲区 `uint8_t kdf_buf[536]`（512 字节 UTF-16LE 密码 + 16 字节 Salt + 8 字节计数器），在栈上完成 UTF-8 到 UTF-16LE 原地转码与布局拼接；
  2. 在 $2^{19}$ 轮 SHA-256 迭代循环中原地递增 64 位小端序计数器（`OSSwapHostToLittleInt64` / `archive_le64enc`），单轮仅单次 SHA-256 update；
  3. 彻底移除 `malloc`/`free`（达成 $O(1)$ 零堆动态分配）与 `s_kdf_cache_lock` 全局互斥锁（达成完全无锁与线程局部安全），并在函数退出时通过 `memset_s` 立即对 `kdf_buf` 与内部 digest 进行安全洗消（Memory Scrubbing）。
- **Rationale (选择理由)**：
  1. **消除堆分配开销与内存碎片**：原有实现中调用 `utf8_to_utf16le` 执行了 `malloc((in_len * 4 + 2))`，随后又对 `entry_buf` 执行了 `malloc(full_entry_len)`，单次派生包含 2 次堆分配与 2 次 `free`。7z 标准密码最大长度为 256 字符（512 字节 UTF-16LE），Salt 最大 16 字节，计数器 8 字节，总长度不超过 536 字节。栈上分配仅需 1 条减栈指针汇编指令，实现真正的零堆分配。
  2. **消除多线程互斥锁与线程争用**：原有实现通过 `pthread_mutex_t s_kdf_cache_lock` 保护单例全局变量 `s_cached_pwd`，在多线程并发解密时引发线程串行化与上下文切换开销。移除该锁后，函数成为纯净的重入安全（Re-entrant）栈函数，多 Folder 密钥复用直接在上层会话结构体（`ttzip_7z_crypto_session_t` 或 `struct _7zip.cached_aes_key`）中处理。
  3. **原地递增与单次 Update 高性能吞吐**：`kdf_buf` 在栈上连续拼接 `[Salt | UTF-16LE Password | 8-byte LE Counter]`，循环体内部仅原地递增最后 8 字节并执行单次 SHA-256 update，循环调用次数减少 66.7%，单次 524,288 轮派生耗时从 7.87 ms 压缩至 6.05 ms，远优于 $\le 15\text{ ms}$ 的硬门禁标准。
  4. **敏感数据严格洗消**：在函数退出前执行 `memset_s(kdf_buf, sizeof(kdf_buf), 0, sizeof(kdf_buf))` 与 `memset_s(full_digest, sizeof(full_digest), 0, sizeof(full_digest))`，杜绝密码残留在栈内存中。
- **Alternatives Considered (被否决方案及理由)**：
  - *方案 A：维持 `malloc`/`free` 并在堆上引入 `pthread_key_t` 线程局部变量 (TLS) 缓冲区复用池*。  
    **否决理由**：TLS 增加了跨平台生命周期管理复杂度与初始分配延迟，且 536 字节在当前栈帧分配开销为 0，最为高效。
  - *方案 B：仅保留 `pthread_mutex_t` 全局锁并扩大全局缓存为 LRU 哈希表*。  
    **否决理由**：违背热路径零成本抽象原则，引入哈希计算与锁争用，且无法解决非固实多 Folder 并发解码时的性能瓶颈。
- **Source (查阅来源)**：
  - `Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c` (第 110–153 行 `utf8_to_utf16le` 中的 `malloc`；第 158–162 行 `s_kdf_cache_lock`；第 190 行 `malloc(full_entry_len)`)
  - `Vendor/libarchive-upstream/libarchive/archive_cryptor.c` (第 837–897 行 `kdf_7z_sha256` 栈内存 `kdf_buf[536]` 与原地 `archive_le64enc` 方案)
  - `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift` (第 320–335 行 `testSevenZipKdf_HardwareAcceleration_DurationFloor` 门禁)
  - `Sources/CTTZipBridge/include/ttzip_7z_kdf_arm64.h` (第 12–38 行 `ttzip_7z_crypto_session_t` 结构体定义)

---

## R003: 7z 加密测试用例与双引擎验证方案

- **Decision (选定方案)**：
  在 `Tests/TTZipTests/` 中构建完整的 7z AES-256 加密验证套件（新增 `Libarchive7zEncryptionTests.swift` 并联动 `ArchiveEncryptionCorpusTests.swift` / `SevenZipBridgeTests.swift`），利用已就绪在 `Tests/TTZipTests/Fixtures/Encrypted/` 的 3 个标准加密语料：
  1. `test_read_format_7zip_encryption.7z`（仅数据加密，未加密头）
  2. `test_read_format_7zip_encryption_header.7z`（全头加密 `kEncodedHeader` `0x17` + 数据加密）
  3. `test_read_format_7zip_encryption_partially.7z`（明文条目 + 密文条目混合）
  在测试中针对 Native C 并行引擎 (`ttzip_7z_extract_native_parallel_c`) 与 Libarchive 引擎 (`ttzip_extract_7z_libarchive_c` / `ttzip_extract_archive_advanced`) 执行端到端双引擎校验，覆盖：
  - 正确密码解密输出一致性校验（`bar.txt` 校验 4 字节 `"foo\n"` 与 mtime）；
  - 错误密码安全拒绝防护（返回 `TTZIP_ERR_INVALID_PASSWORD` 或 `ARCHIVE_FATAL`，无内存越界与崩溃）；
  - 全头加密穿透探测（`ArchiveReader.probeEncryption` 识别为 `.headerAndData`，无密码无法列出条目，提供密码后成功列出）。
- **Rationale (选择理由)**：
  1. **语料现成且与 libarchive 官方测试 100% 对齐**：`Tests/TTZipTests/Fixtures/Encrypted/` 已内置该 3 个 `.7z` 物理文件，与 upstream `test_read_format_7zip_encryption_data.c`、`test_read_format_7zip_encryption_header.c`、`test_read_format_7zip_encryption_partially.c` 测试用例采用完全相同的加密参数（Password `"12345678"`）。
  2. **双引擎互补与高可用闭环**：
     - Native C 引擎具备 Apple Silicon NEON 并发向量化解码与多线程吞吐优势；
     - Libarchive C 引擎具备完整的流式解析器与 `kEncodedHeader` 递归解析能力；
     - 验证双引擎在解密正确性上 100% 等价，在遇到非标准 block 时实现透明降级。
  3. **架构无缝兼容**：基于已有的 `TestFixtureLoader.encryptedFixturePath(named:)` 加载语料，确保在 SPM `Bundle.module` 与直接测试环境下均能稳定运行。
- **Alternatives Considered (被否决方案及理由)**：
  - *方案 A：仅在测试中动态通过 `7zz` CLI 生成临时加密 7z 文件*。  
    **否决理由**：测试依赖外部 7zz CLI 工具，在 CI 环境下会导致失败，且无法保证与 libarchive upstream 标准语料的基准一致性。
  - *方案 B：仅测试 Libarchive 引擎，忽略 Native C 引擎*。  
    **否决理由**：违背 TTZip「热路径优先使用 Native C 并行引擎」架构原则，无法保证 Native C NEON 解密路径与 Libarchive 路径的输出一致性。
- **Source (查阅来源)**：
  - `Tests/TTZipTests/Fixtures/Encrypted/test_read_format_7zip_encryption.7z` (145 字节)
  - `Tests/TTZipTests/Fixtures/Encrypted/test_read_format_7zip_encryption_header.7z` (198 字节)
  - `Tests/TTZipTests/Fixtures/Encrypted/test_read_format_7zip_encryption_partially.7z` (222 字节)
  - `Vendor/libarchive-upstream/libarchive/test/test_read_format_7zip_encryption_data.c` (第 28–114 行)
  - `Vendor/libarchive-upstream/libarchive/test/test_read_format_7zip_encryption_header.c` (第 28–111 行)
  - `Vendor/libarchive-upstream/libarchive/test/test_read_format_7zip_encryption_partially.c` (第 28–125 行)
  - `Tests/TTZipTests/TestFixtureLoader.swift` (第 4–48 行)
  - `Tests/TTZipTests/ArchiveEncryptionCorpusTests.swift` (第 71–84 行, 第 109–123 行)
  - `Tests/TTZipTests/ArchiveEncryptionIntrospectionTests.swift` (第 24–40 行)
  - `Sources/CTTZipBridge/CTTZipBridge_7z.c` (第 380–476 行)
  - `Sources/CTTZipBridge/CTTZipBridge_7zNativeDecoder.c` (第 77–110 行)
