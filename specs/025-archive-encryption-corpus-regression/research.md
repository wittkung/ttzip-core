# Phase 0 Research: Corpus-Driven Archive Encryption Regression & Acceptance Suite

## R001: 静态测试语料加载机制与打包格式选型 (Fixture Loading & Storage Strategy)

- **Decision**: 采用 **SPM 资源包 (`resources: [.copy("Fixtures")]` + `Bundle.module`) 与动态合成 (`TestFileGenerator`)** 双轨分层方案。静态只读语料（ZIP WinZip AES-128/256、7z Header/Data AES-256、RAR4/RAR5、混合加密包）集中纳管于 `Tests/TTZipTests/Fixtures/Encrypted/`，由轻量级 `TestFixtureLoader` 在 XCTest 运行时通过 `Bundle.module.url(...)` 获取物理文件路径，直通 C 桥接引擎的 `open()` / `mmap()` 零堆拷贝接口。
- **Rationale**:
  1. SPM 原生标准化：Swift 6.0 官方标准资源声明，严格保持原样二进制字节布局与目录结构，不篡改文件拓展名。
  2. 零内存复制与零 I/O 浪费：资源在编译阶段部署于测试 Bundle 中，直接提供物理路径给 C 引擎，无需在运行时动态写入临时磁盘或在堆上分配 `Data(base64Encoded:)`。
  3. 零外部网络依赖：测试随 Git 仓库版本受控（控制在单文件 < 100KB 的微型样本），支持在断网或隔离 CI 环境下毫秒级执行。
- **Alternatives Considered**:
  - *Base64 字符串硬编码于 Swift 测试源文件*：被否决。大量 Base64 字面量会拖慢 Swift 编译解析速度，且运行时必须额外分配堆内存解码并写入临时磁盘，违背热路径低开销原则；且无法使用外部十六进制或系统工具直接检视。
  - *通过 `#filePath` 相对源码路径定位*：被否决。在沙盒化测试或产物分离的 CI 容器中，源码物理路径不可达或发生重定位，导致 `FileNotFound`。
- **Source**:
  - `Package.swift#L89-92` & `Package.swift#L110-119`
  - `Tests/TTZipTests/TestFileGenerator.swift#L5-78`
  - `Sources/CTTZipBridge/CTTZipExtract.c#L37-60`

---

## R002: WinZip AES 与 7z/RAR5 加密规范与认证校验机制 (Encryption Matrix & Authentication Verification)

- **Decision**: 
  1. **WinZip AES**：采用**双阶段流水线认证**。阶段一通过 PBKDF2 派生的 2-byte Password Verification Value (PVV) 快速判定密码正确性（不匹配即刻返回 `TTZIP_ERR_INVALID_PASSWORD`）；阶段二在 AES-CTR NEON 解密与末尾 10-byte HMAC-SHA1 校验时，若 HMAC 失败则准确归类为 `TTZIP_ERR_CORRUPT_HEADER` / `TTZIP_ERR_CORRUPT_DATA`。
  2. **7z AES-256**：原生支持 `kEncodedHeader` (`0x17`) 全头加密递归流解析与 ARM64 NEON SHA-256 KDF 加速。头加密归档首块解密后校验 7z 语法树结构；数据流加密归档结合 LZMA2 首块字典状态与最终解压 CRC32 精准派发错误码。
  3. **RAR4 / RAR5**：RAR5 启用 `HEAD_CRYPT` / `EX_CRYPT` 内置的 8 字节 `PSWCHECK` SHA-256 快速摘要比对，在分配大块解压缓冲区前直接实现 0 堆分配的无损密码验证；RAR4 维持 Header CRC16 与 UnRAR 分级错误拦截。
- **Rationale**:
  1. 零堆分配极速响应：WinZip AES 的 PVV 与 RAR5 的 PSWCHECK 均可在执行高开销并行解密和 Decompressor 内存分配前（耗时 < 1ms）判定密码正确性。
  2. 严格区分密码错误与数据损坏：避免向用户误报“文件损坏”，提高错误语义精确度。
  3. 硬件加速性能守恒：7z 循环 SHA-256 KDF 耗时稳定满足 Apple Silicon $\le 15\text{ms}$ 硬门禁。
- **Alternatives Considered**:
  - *在 WinZip AES 中直接使用 Decompressor 语法报错代替 10 字节 HMAC-SHA1 校验*：被否决。对于 Store 模式文件，错误的 AES 密钥解密出的乱码在 Store 解包时不会报错，不校验 HMAC 会导致直接写出乱码文件并误报成功。
  - *对 7z 全头加密使用 libarchive 通用流式回退*：被否决。上游 libarchive 遇到 `kEncodedHeader` 会直接返回 `ARCHIVE_FATAL`，导致全头加密 7z 无法解压。
- **Source**:
  - `Vendor/libarchive-upstream/libarchive/archive_read_support_format_zip.c#L144-146`, `#L3273-3353`
  - `Vendor/libarchive-upstream/libarchive/test/test_read_format_zip_winzip_aes.c#L59-133`
  - `Vendor/libarchive-upstream/libarchive/archive_read_support_format_7zip.c#L105`, `#L3929-4028`
  - `Sources/CTTZipBridge/CTTZipBridge_Crypto.c#L405-459`, `#L566-616`
  - `Sources/CTTZipBridge/CTTZipExtract.c#L174-180`, `#L258-278`

---

## R003: 三级加密状态自省与 Swift 错误模型对齐 (3-Tier Encryption Introspection & Error Handling)

- **Decision**: 
  1. **C 桥接层自省直通**：扩展归档遍历回调，透传 `(is_data_encrypted, is_metadata_encrypted, enc_algo)`，并在 `CTTZipBridge_Archive.c` 中绑定 libarchive 的 `archive_entry_is_data_encrypted` 与 `archive_entry_is_metadata_encrypted`。
  2. **Swift 强类型枚举与实体升级**：定义 `ArchiveEncryptionTier` (`none`, `dataOnly`, `headerAndData`, `unsupported`)，并在 `ArchiveEntry` 中显式暴露 `isEncrypted`, `isDataEncrypted`, `isMetadataEncrypted`, `encryptionMethod`。
  3. **无密码快速探测协议**：在 `ArchiveReading` 协议中新增 `probeEncryption(archivePath:) async throws -> ArchiveEncryptionTier`，废除针对加密归档创建临时目录进行试探性解压的重型 I/O 回退。
  4. **Swift 错误模型精准分化**：升级 `ArchiveError`，明确区分 `passwordRequired(archivePath:tier:)` 与 `wrongPassword(archivePath:)`。
- **Rationale**:
  1. 极致性能：纯内存解析表头与标志位（< 1ms），彻底消除临时目录解压开销。
  2. UI 深度适配：UI 层（Miller 栏、列表视图）可直接依据 `entry.isEncrypted` 绑定锁定图标，并在用户双击 Tier 1 归档时秒级展示目录树，双击 Tier 2 归档时精准提示前置密码输入。
  3. 概念 1:1 对齐：与 libarchive C 规范概念严谨对齐。
- **Alternatives Considered**:
  - *仅在 Swift 上层通过试探性解压捕获错误码*：被否决。试探性解压涉及磁盘 I/O 和内存分配，违背零多余系统调用原则，且无法在用户解压前提供条目级图标绑定。
  - *将加密状态扁平化为单一布尔值 `isEncrypted: Bool`*：被否决。无法区分数据加密与头部加密，导致 UI 无法预判应直接展示文件树还是弹出全屏密码输入框。
- **Source**:
  - `Vendor/include/archive.h#L397-412`
  - `Vendor/include/archive_entry.h#L330-332`
  - `Sources/TTZipCore/ArchiveEntry.swift#L3-39`
  - `Sources/TTZipCore/ArchiveReader.swift#L4-28`, `#L98-198`
  - `Sources/CTTZipBridge/CTTZipBridge_Archive.c#L45-93`
  - `Sources/CTTZipBridge/CTTZipParser.c#L111-143`
