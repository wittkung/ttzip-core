# Feature Specification: libarchive 级防御性安全与零分配热路径加固规范 (Defensive Security & Zero-Allocation Hardening)

**Feature Directory**: `specs/038-libarchive-defensive-security-and-zero-allocation-hardening`  
**Created**: 2026-08-16  
**Status**: Draft  
**Input**: "针对代码审查中发现的 5 大薄弱点（Zip Slip 符号链接防御盲区、超大 Solid 归档内存耗尽、热路径 Data(count:) 隐式零填充开销、C 句柄魔数清零与敏感内存擦除、跨语言整型窄化 Clamp）进行系统性加固与工程规范重构"

---

## Clarifications

### Session 2026-08-16
- **Q1 (Zip Slip 防御层级)**: 路径清洗是在 Swift 侧还是 C 侧执行？
  - **Resolution**: 采用双层纵深防御：(1) Swift 侧在 `SecurityScanner.swift` 中增加前置 `sanitizePath`，拦截 `..` 与绝对路径；(2) C 桥接层在 `archive_write_disk_set_options` 中开启 `ARCHIVE_EXTRACT_SECURE_SYMLINKS` 与 `ARCHIVE_EXTRACT_SECURE_NOABSOLUTEPATHS`。
- **Q2 (零分配热路径改造方案)**: 如何替代 `LibdeflateCAdapter` 中的 `Data(count:)`？
  - **Resolution**: 使用 `UnsafeMutablePointer<UInt8>.allocate(capacity:)` 分配未初始化内存块，并在完成后通过 `Data(bytesNoCopy:count:deallocator:)` 包装，消除内核物理页清零开销。
- **Q3 (密码擦除函数选择)**: 使用什么函数进行敏感内存清零？
  - **Resolution**: 在 macOS 平台优先使用系统内建安全的 `memset_s`，确保不会被编译器 dead-store 优化移除。


## User Scenarios & Testing *(mandatory)*

### User Story 1 - 解压管道 Zip Slip 与符号链接穿透防御加固 (Priority: P1)

作为终端用户和系统安全管理员，我在解压不受信任的第三方归档时，系统必须自动执行路径规范化清洗，开启 `ARCHIVE_EXTRACT_SECURE_SYMLINKS` 与 `ARCHIVE_EXTRACT_SECURE_NOABSOLUTEPATHS`，拦截任何包含 `../` 相对路径、根目录绝对路径或中间指向外部目标的软链接，彻底杜绝目录逃逸与任意文件覆盖攻击。

**Why this priority**: 路径穿越是归档软件最致命的安全漏洞（CVE 高危），必须前置物理阻断。

**Independent Test**: 运行 `swift test --filter SecurityScannerTests`，使用包含 `../`、`/etc/passwd` 以及中间符号链接的恶意测试归档执行解压，断言所有越权写操作被 100% 拦截并返回 `TTZipError`。

**Acceptance Scenarios**:
1. **Given** 包含 `../../sensitive.txt` 的恶意归档，**When** 调用解压管道，**Then** 引擎直接阻断并抛出安全告警，严禁写出目标目录之外。
2. **Given** 包含指向宿主目录软链接后紧随目标写入的归档，**When** 执行解压，**Then** 逐级符号链接校验（`check_symlinks`）触发 `ELOOP` 阻断。

---

### User Story 2 - 消除热路径隐式内核零填充与内存分配优化 (Priority: P2)

作为性能敏感型用户，我在进行大规模多线程解压和压缩（ZIP/7z/Deflate）时，引擎热循环体中必须使用未初始化裸指针（`allocate(capacity:)`）与享元页池（`MemoryPageFlyweightPool`），消除所有的 `Data(count:)` 内核物理页零填充开销（Zero-Fill Page Faults），使内存复用率和 CPU 吞吐达到硬件极限。

**Why this priority**: 零成本抽象与热路径零分配是 TTZip 的宪法级不变量，消除内核中断可显著提升 GB 级数据流吞吐。

**Independent Test**: 运行 `swift test --filter XCTestPerformanceMeasureTests`，验证 ZIP 和 Deflate 解压吞吐达到门禁标准（>= 6500 MB/s）。

**Acceptance Scenarios**:
1. **Given** `LibdeflateCAdapter` 与 `ZipMemoryEngine`，**When** 执行大块解压，**Then** 缓冲区分配完全走未初始化裸指针，避免 `Data(count:)` 产生清零开销。

---

### User Story 3 - C 桥接层句柄魔数清零与敏感内存安全擦除 (Priority: P3)

作为底层基础设施开发者，C 桥接层所有导出的结构体首字段必须嵌入 Magic 校验魔数，且在释放（`free`）之前必须显式清零（`magic = 0`），杜绝 Use-After-Free；所有解密密码与 UTF-16 派生缓冲区在释放前必须调用 `explicit_bzero` 擦除。

**Why this priority**: 杜绝内存损坏漏洞与内存转储（Core Dump）中的明文密码泄漏。

**Independent Test**: 审查并测试 C 桥接层句柄生命周期与内存安全。

**Acceptance Scenarios**:
1. **Given** 分配的 C 句柄，**When** 释放句柄，**Then** 魔数先被清零再调用 `free()`。
2. **Given** 加密解压流程，**When** 密码使用完毕，**Then** 内存被立即清零。

---

## Edge Cases

- **超长合法路径**：路径深度超过 1024 字节时，动态分配安全缓冲区，避免固定 `char buf[1024]` 溢出截断。
- **64 位整型窄化转换**：在 32 位与 64 位转换边界，所有文件偏移量与块大小必须经过 `SSIZE_MAX` Clamp。

---

## Requirements *(mandatory)*

- **FR-001**: 必须在 `Sources/CTTZipBridge/CTTZipBridge_Archive.c` 中开启 `ARCHIVE_EXTRACT_SECURE_SYMLINKS` 与 `ARCHIVE_EXTRACT_SECURE_NOABSOLUTEPATHS`。
- **FR-002**: 必须在 `Sources/TTZipCore/SecurityScanner.swift` 中增加路径穿越清洗（`sanitizePath`）与绝对路径/符号链接深度检测。
- **FR-003**: 必须重构 `Sources/TTZipCore/Adapters/LibdeflateCAdapter.swift`，消除 `Data(count:)` 隐式零填充，改用未初始化内存缓冲。
- **FR-004**: 必须在 `Sources/CTTZipBridge/CTTZipBridge_Crypto.c` 中对密码缓冲区在释放前强制调用 `memset_s` / `explicit_bzero`。
- **FR-005**: 必须在跨语言数据边界增加 `SSIZE_MAX` Clamp 保护。

---

## Success Criteria *(mandatory)*

- **SC-001**: Zip Slip 与符号链接穿越测试 100% 拦截通过。
- **SC-002**: `LibdeflateCAdapter` 消除所有热路径 `Data(count:)`。
- **SC-003**: 密码内存安全清零覆盖率 100%。
- **SC-004**: 全量单元测试与性能门禁测试 100% 绿灯通过，零倒退。
