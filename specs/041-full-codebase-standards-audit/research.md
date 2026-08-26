# Research Notes: TTZip 全代码库深度规范与系统级不变量审计 (Research & Architectural Invariants)

**Feature Directory**: `specs/041-full-codebase-standards-audit`  
**Created**: 2026-08-17  
**Status**: Completed  
**Inputs & Grounding**: 基于 3 个专项子 Agent 对全库 170+ 源文件（C Bridge, Swift Core, App/CLI, Tests）的逐行物理静态与动态边界分析。

---

## 1. C 桥接层与底层引擎系统级不变量审查 (Research Item R001)

### R001.1: 敏感凭据与派生密钥死存储消除 (DSE) 防御与物理安全擦除
- **Decision**: 在 `Sources/CTTZipBridge/` 下所有涉及密码哈希、PBKDF2/KDF 派生密钥与解密中间状态的函数中，彻底废除普通的 `memset`，全面采用 C11 标准 `memset_s` / macOS `explicit_bzero` 并在所有分支（含早期错误退出）强制擦除。
- **Rationale**: Clang 编译器（`-O2`/`-O3`）在 Dead-Store Elimination 阶段会将离开作用域前的局部变量 `memset` 优化消除，导致明文密码与 256 位 AES 密钥残留在栈与堆中（CWE-14 / CWE-214）。`memset_s` 具有 volatile 写入语义，保证物理内存清零。
- **Alternatives Considered**:
  - *使用 `#pragma optimize("", off)` 局部禁用优化*：被否决。属于编译器非标扩展，降低热路径执行效率且不具备跨平台保证。
  - *依赖操作系统进程退出时回收*：被否决。严重违反安全密码学规范，可能被同进程内其他任务读取。
- **Source**: 
  - `Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c:181, 206-207`
  - `Sources/CTTZipBridge/CTTZipBridge_Crypto.c:285-291, 419-455`
  - `Sources/CTTZipBridge/CTTZipBridge_ZipWrite.c:232, 282, 315`

### R001.2: 原生解压与落盘路径的 Zip-Slip 防护与 O_NOFOLLOW 物理防御
- **Decision**: 在 `ttzip_tar_zstd_direct.c`、`CTTZipBridge_7zNativeDecoder.c` 与 `CTTZipExtract.c` 中，强制开启全套 POSIX 安全标志；所有 `open` 写入调用必须包含 `O_NOFOLLOW`，并引入延后 Fixup 倒序回写机制。
- **Rationale**: 缺失 `O_NOFOLLOW` 时，归档中先行的软链接可将后续条目重定向写入 `/etc` 等系统关键路径（TOCTOU 漏洞）。延后 Fixup 确保目录先以 `0700` 建立，全部写入完成后按深度从深到浅回写权限与 mtime，防止只读目录锁死后续写入。
- **Alternatives Considered**:
  - *仅在 Swift 层通过正则拦截 `../`*：被否决。违反 Invariant-First 铁律，纯上层正则无法防御解压过程中动态创建的符号链接跨条目穿越。
- **Source**: 
  - `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c:665, 680-686`
  - `Sources/CTTZipBridge/CTTZipBridge_7zNativeDecoder.c:169, 221`
  - `Sources/CTTZipBridge/CTTZipExtract.c:217, 283, 317`
  - `Sources/CTTZipBridge/CTTZipBridge_7z.c:404`
  - `Sources/CTTZipBridge/ttzip_tar_native.c:265`

### R001.3: Solid 压缩与 LZFSE 解码内存模型重构 (Stream-First)
- **Decision**: 废除 Solid 压缩一次性分配全量未压缩与压缩数据、以及 LZFSE 8 倍预分配的模型，改造为固定 64MB~128MB 分块流式滑动窗口管道。
- **Rationale**: 处理 100GB 归档时，全量分配直接申请超过 200GB RAM，必然触发内核 OOM 崩溃；分块滑动窗口将内存常驻严格约束在 $O(1)$。
- **Alternatives Considered**:
  - *使用 anonymous mmap 依赖虚拟内存换页*：被否决。海量匿名换页引发内核页中断风暴（Zero-Fill Faults），大幅劣化吞吐。
- **Source**: 
  - `Sources/CTTZipBridge/CTTZipBridge_7zSolid.c:57, 126`
  - `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c:202, 297`
  - `Sources/CTTZipBridge/CTTZipBridge_LZFSE.c:83, 128-133`
  - `Sources/CTTZipBridge/ttzip_7z_block_decoder.c:32, 103`

### R001.4: C 句柄结构体 Magic 哨兵与 64 位整型溢出防御
- **Decision**: 在全库所有 C 句柄结构体首字段统一嵌入 `uint32_t magic`，在 `free()` 前强制写 `magic = 0`；所有 64 位向 `size_t` 转换前必须通过 `SSIZE_MAX` Clamp，算术运算调用 `__builtin_add_overflow` / `__builtin_mul_overflow`。
- **Rationale**: 将 Use-After-Free 和 Double-Free 转换为可确定性捕获的错误；防止外部恶意构造的 7z VarInt 触发整数溢出与堆越界。
- **Alternatives Considered**:
  - *仅依赖 AddressSanitizer*：被否决。ASan 仅用于测试期，生产环境必须具备内建确定性自愈与拦截能力。
- **Source**: 
  - `Sources/CTTZipBridge/ttzip_7z_header_parser.c:57, 110, 201, 235, 266, 294`
  - `Sources/CTTZipBridge/CTTZipStreamCoder.c:75`
  - `Sources/CTTZipBridge/CTTZipBridge_GzParallel.c:54`

---

## 2. Swift 核心管道与 28 大设计模式数据平面合规审查 (Research Item R002)

### R002.1: 7z CBC 并发加解密模型校正
- **Decision**: 将 `SevenZipCryptoEngine.processParallelAES256` 的加密路径改为严格串行流式加密（或块间前置传递最后 16 字节密文作为下一块 IV）；解密路径保留已验证的 Ciphertext Block N-1 IV 截取并发机制。
- **Rationale**: CBC 模式下第 N 块的 IV 必须是第 N-1 块的最终密文。现有实现在 `encrypt == true` 时所有并发分块均重复使用初始 `iv`，不仅导致生成的 7z 归档严重损坏无法解压，还构成了严重的分块 IV 重用密码学漏洞。
- **Alternatives Considered**:
  - *分块各自生成随机 IV 写入自定义头*：被否决。违背 7z 官方归档容器规范，导致主流工具无法识别。
- **Source**: `Sources/TTZipCore/SevenZip/SevenZipCryptoEngine.swift:44-114`

### R002.2: Solid Block 并发解压产物落盘修复
- **Decision**: 修复 `SevenZipBlockParallelDecompressor.decompressSolidBlocksConcurrently`，补齐解压产物回传或向目标目录写盘逻辑，杜绝解压数据后直接 `free(dstRawPtr)` 丢弃的幽灵缺陷。
- **Rationale**: 原逻辑解压后直接释放缓冲区未写盘，导致 7z 固实分块解压流程看似返回成功但实际产物为空。
- **Alternatives Considered**:
  - *移除并发 Solid 解压改用纯 C 引擎*：被否决。保留 Swift 调度层能力，修复数据回传通道。
- **Source**: `Sources/TTZipCore/SevenZip/SevenZipBlockParallelDecompressor.swift:41-53`

### R002.3: 密码恢复引擎纯内存 Header 探针重构
- **Decision**: 废除 `PasswordRecoveryEngine` 每次尝试均全量解包到磁盘的逻辑，改为纯内存计算派生密钥并比对 Local Header / 7z Encrypted Header 的校验字节。
- **Rationale**: 避免上万次字典密码尝试对 SSD 造成 TB 级写入磨损，将单次密码尝试耗时从数十毫秒降低至微秒级。
- **Alternatives Considered**:
  - *仅解压首个小文件到磁盘*：被否决。仍有文件系统 I/O 与临时目录锁争用开销。
- **Source**: `Sources/TTZipCore/PasswordRecoveryEngine.swift:146-172`, `Sources/TTZipCore/TemplateMethod/PasswordRecoveryEngineTemplate.swift:126-152`

### R002.4: CUnsafeBufferAdapter 递归展开为非递归安全绑定
- **Decision**: 重构 `CUnsafeBufferAdapter.withCStringsArray`，使用局部连续数组一次性构造 `[UnsafePointer<CChar>?]` 传递给 body，彻底消除嵌套闭包尾递归。
- **Rationale**: Swift 闭包无法进行尾调用优化，处理 10,000+ 文件列表时产生数万层调用栈直接触发 Stack Overflow 崩溃。
- **Alternatives Considered**:
  - *手动 `strdup` / `malloc` 并在完成后循环 `free`*：被否决。存在内存泄漏隐患，破坏 Swift RAII 作用域安全。
- **Source**: `Sources/TTZipCore/Adapters/CUnsafeBufferAdapter.swift:20-81`

### R002.5: 热路径零成本抽象与锁优化
- **Decision**:
  1. `MemoryPageFlyweightPool`: 移除强制解包 `!` 与借出时的冗余 64KB `memset`；
  2. `ZipParallelWriter`: 移除 `concurrentPerform` 内部的高频 `lock.lock()`，改用原子累计 + 60Hz 采样节流；
  3. `ZipBlockParallelCompressor` / `ZipMemoryEngine`: 移除循环中的 `Data(count:)`，改用裸指针分配 + `Data(bytesNoCopy:)`。
- **Rationale**: 消除内核零填充缺页中断与多核 CPU 串行锁争用，捍卫历史最优性能门禁。
- **Alternatives Considered**:
  - *使用信号量节流*：被否决。信号量引入额外内核系统调用。
- **Source**: 
  - `Sources/TTZipCore/Flyweights/MemoryPageFlyweightPool.swift:96-124`
  - `Sources/TTZipCore/Zip/ZipParallelWriter.swift:117-129`
  - `Sources/TTZipCore/Zip/ZipBlockParallelCompressor.swift:21, 45`

---

## 3. 测试套件真实预言机效力与表现层架构隔离审查 (Research Item R003)

### R003.1: 变异模糊测试伪通过修复 (Crash-First Fuzzing)
- **Decision**: 重构 `ArchiveMutationFuzzTests`，剔除将变异二进制流错误传入 `UUDecoder` 的逻辑，直接传入 `ArchiveExtractor` / `ArchiveReader` 执行解压鲁棒性测试，并在传入前将样本落盘至沙盒调试目录。
- **Rationale**: 原测试中 `UUDecoder` 因非 ASCII 文本解析失败直接返回 `nil`，导致 100 次变异测试均在第一步被捕获为“安全拦截”，实际底层解压引擎从未被模糊测试覆盖。
- **Alternatives Considered**:
  - *在 UUDecoder 前增加格式校验*：被否决。模糊测试的目标是解压引擎而非 UU 文本解码器。
- **Source**: `Tests/TTZipTests/ArchiveMutationFuzzTests.swift:61-78`

### R003.2: 黄金语料库解压执行闭环与 16 大格式 CVE 样本扩充
- **Decision**: 在 `ArchiveGoldenCorpusTests` 中将解码得到的二进制数据实际送入 `ArchiveExtractor` 解析与解压断言，并扩充至 16 大格式真实 CVE 语料。
- **Rationale**: 原测试仅断言了 `UUDecoder.decode` 成功，未执行引擎解压；且现有样本仅覆盖 4 种格式，未形成有效历史回归预言机。
- **Alternatives Considered**:
  - *另建 GoldenExtractTests*：被否决。统一在 GoldenCorpus 套件中完成解码与解压闭环。
- **Source**: `Tests/TTZipTests/ArchiveGoldenCorpusTests.swift:39-72`

### R003.3: 系统原生 CLI 双向差分测试矩阵完善
- **Decision**: 在 `SystemDifferentialTests` 中建立“系统打包 ➔ TTZip 解压校验”与“TTZip 打包 ➔ 系统工具解压校验”的双向互操作性断言，接入 `/usr/bin/tar` 与 `/usr/bin/unzip`。
- **Rationale**: 消除“自产自销”同义反复，确保 TTZip 生成的归档完全符合 POSIX 与 Info-ZIP 行业黄金标准。
- **Alternatives Considered**:
  - *仅单向测试系统工具解压 TTZip 产物*：被否决。无法保证对系统工具生成归档的解析兼容性。
- **Source**: `Tests/TTZipTests/SystemDifferentialTests.swift:27-65`

### R003.4: 表现层 SecureField 全面替换为 TTSecureTextField (IME 兼容)
- **Decision**: 将 7 处 Popover、Sheet 与设置视图中的 `SecureField` 替换为自定义明暗切换输入框 `TTSecureTextField`。
- **Rationale**: macOS 原生 `SecureField` 在附着窗口（Popover/Sheet）激活时会触发 TSM 锁定，导致系统中文输入法全局卡死。
- **Alternatives Considered**:
  - *保留 SecureField 并弹出警告*：被否决。严重违反用户体验与项目规范。
- **Source**: 
  - `Sources/TTZipApp/Views/Components/CompressAdvancedOptionsSectionView.swift:158`
  - `Sources/TTZipApp/Views/Components/CompressIntegratedConfigSectionView.swift:171`
  - `Sources/TTZipApp/Views/ExtractModalView.swift:124`
  - `Sources/TTZipApp/Views/PasswordVaultPopoverView.swift:39`
  - `Sources/TTZipApp/Views/PasswordVaultView.swift:51, 61, 98`
  - `Sources/TTZipApp/Views/PresetWorkspaceView.swift:122`
  - `Sources/TTZipApp/Views/SettingsView.swift:49`

### R003.5: 跨层依赖隔离与 C 符号收敛
- **Decision**: 移除 `TTZipApp.swift` 与 `TTZipCLIApp.swift` 中的 `import CTTZipBridge`，在 `TTZipCore` 提供统一生命周期初始化 API；移除 `TTZipProcessExecutor.swift` 中的 `@_exported import CTTZipBridge`。
- **Rationale**: 严格维护分层架构单向依赖（Layer 3 -> Layer 2 -> Layer 1 -> Layer 0），杜绝底层 C ABI 泄漏至表现层。
- **Alternatives Considered**:
  - *在 Package.swift 中为 TTZipApp 显式添加 CTTZipBridge 依赖*：被否决。破坏分层封装。
- **Source**: 
  - `Sources/TTZipApp/TTZipApp.swift:4, 66, 76`
  - `Sources/TTZipCLI/TTZipCLIApp.swift:3, 9, 21`
  - `Sources/TTZipCore/Utilities/TTZipProcessExecutor.swift:3`
