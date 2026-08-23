# Feature Specification: libarchive 黄金预言机语料库、变异模糊测试与系统差分测试工程落地 (libarchive Golden Oracle & Fuzz Integration)

**Feature Directory**: `specs/037-libarchive-golden-oracle-and-fuzz-integration`  
**Created**: 2026-08-16  
**Status**: Draft  
**Input**: "将从 libarchive 学到的黄金预言机测试哲学、UUEncode 历史缺陷语料库、变异模糊测试与跨系统差分测试在 TTZip 代码库中完整工程落地"

---

## Clarifications

### Session 2026-08-16
- **Q1 (UUDecoder 架构选型)**: `UUDecoder` 应在 Swift 侧还是 C 侧实现？
  - **Resolution**: 在 Swift 侧实现轻量、零依赖的流式 `UUDecoder` 工具类（并可被测试套件与 CLI 直接复用），利用纯 Swift 值类型 `Data` 处理，单文件解析速度达 100+ MB/s。
- **Q2 (黄金样本精选集范围)**: 优先引入哪些具有代表性的 upstream 历史缺陷样本？
  - **Resolution**: 优先引入 4 大经典样本：(1) `test_compat_zip_3.zip.uu`（WinZip length-at-end 兼容）；(2) `test_compat_gtar_2.tar.uu`（GNU tar base256 大数值 UID/GID）；(3) `test_read_format_7zip_malformed.7z.uu`（7z 畸形条目一致性防御）；(4) `test_read_format_rar5_pointer.rar.uu`（RAR5 指针边界）。
- **Q3 (模糊测试迭代与性能平衡)**: 每次测试运行多少次变异迭代？
  - **Resolution**: 默认在常规单元测试中运行 100 次变异迭代（耗时 < 0.5s），在完整回归测试门禁下支持扩展至 500 次迭代。


## User Scenarios & Testing *(mandatory)*

### User Story 1 - UUEncode 黄金缺陷语料库与解码回归套件 (Priority: P1)

作为核心引擎开发者，我需要将 upstream 真实积累的经典历史缺陷归档（涵盖 Zip64 4GiB 边界、RAR5 指针悬垂、7z 畸形头部、GNU tar base256 等）作为纯文本 `.uu` 资产引入仓库，并在测试中通过内存/流式解码器动态还原并进行解压回归测试，以确保 TTZip 对工业级边缘场景和已知 CVE 漏洞具备 100% 免疫能力。

**Why this priority**: 真实历史缺陷样本是检验解压管道健壮性的最高客观预言机，避免 Git 二进制膨胀的同时建立坚不可摧的回归防线。

**Independent Test**: 运行 `swift test --filter ArchiveGoldenCorpusTests`，验证所有黄金缺陷样本均能被正确识别、安全解析或平稳抛出预期错误，零崩溃。

**Acceptance Scenarios**:
1. **Given** UUEncoded 历史缺陷样本集（`.uu`），**When** 运行 `ArchiveGoldenCorpusTests`，**Then** 测试套件能够在内存中快速解码并驱动引擎解压，验证解压内容 SHA-256 或格式边界一致性。
2. **Given** 畸形 Header 或超大文件数样本，**When** 引擎执行解析，**Then** 引擎必须安全熔断并抛出 `TTZipError`，严禁触发 SIGSEGV、野指针越界或 OOM 崩溃。

---

### User Story 2 - In-Process 变异模糊测试与崩溃优先转储门禁 (Priority: P2)

作为质量保证与安全工程师，我需要一个内置在测试套件中的轻量级变异模糊测试门禁（In-Process Mutation Fuzzer），对有效归档注入 ~1% 伪随机字节扰动，并在调用解析器之前**先将变异后样本落盘至沙盒调试文件**，随后进行“全量解压”与“仅遍历 Header（Skip Body）”双模式消费测试，验证解析器与状态机的容错恢复能力。

**Why this priority**: 能够在 CI 和日常回归中秒级发现边界崩溃、死循环和未定义行为，并在崩溃发生的第一时间留存现成的最小复现文件。

**Independent Test**: 运行 `swift test --filter ArchiveMutationFuzzTests`，执行数百次随机变异迭代，断言引擎对任意破坏数据均保持进程级稳定。

**Acceptance Scenarios**:
1. **Given** 合法 ZIP/7z/TAR 归档，**When** 注入 1% 随机变异字节并在解析前转储至 `reproducer.bin`，**Then** 引擎全解压或跳过数据块过程中平稳处理，零段错误，零死循环。
2. **Given** 连续 500 次变异迭代，**When** 运行模糊测试，**Then** 100% 迭代均以成功解析或正常捕获异常退出。

---

### User Story 3 - macOS 系统原生工具跨进程双向差分测试 (Priority: P3)

作为兼容性架构师，我需要建立与 macOS 系统原生 CLI 工具（`/usr/bin/tar`、`/usr/bin/unzip`、`gzip`）的双向差分测试套件，断言 TTZip 生成的归档能够被系统原生工具无损解压，且系统原生工具生成的归档能被 TTZip 正确读取。

**Why this priority**: 消除“自产自销”的测试盲区，确保生成的归档与 macOS 原生生态及第三方工具 100% 互操作。

**Independent Test**: 运行 `swift test --filter SystemDifferentialTests`，验证双向互操作与 SHA-256 逐字节一致性。

**Acceptance Scenarios**:
1. **Given** TTZip 压缩生成的 ZIP 和 TAR 归档，**When** 调度系统 `/usr/bin/unzip` 或 `/usr/bin/tar` 解压，**Then** 提取出的文件内容与原始文件 SHA-256 完全匹配。
2. **Given** 系统原生工具创建的归档，**When** 使用 TTZip 引擎解压，**Then** 解压产物逐字节对齐。

---

## Edge Cases

- **极度截断的畸形流**：输入数据仅剩 1~3 字节即遭遇 EOF，断言微缓冲 reader 不会发生 `avail < min` 导致的野指针解引用。
- **解密重试与炸弹攻击**：连续密码错误或声明解压大小与实际字节数严重不符，断言熔断器立即中断。
- **沙盒文件清理**：变异测试在测试成功后自动清理调试文件，若发生断言失败则完整保留现场。

---

## Requirements *(mandatory)*

- **FR-001**: 必须实现纯 Swift 的轻量级 `UUDecoder`（UUEncode 文本解码器），支持将 `.uu` 文本字符串流式还原为原始二进制 `Data`。
- **FR-002**: 必须在 `Tests/TTZipTests/Fixtures/GoldenCorpus/` 中建立从 libarchive upstream 精选的经典历史缺陷与兼容性 `.uu` 语料库。
- **FR-003**: 必须实现 `ArchiveGoldenCorpusTests.swift`，对语料库样本进行全量回归校验。
- **FR-004**: 必须实现 `ArchiveMutationFuzzTests.swift`，具备 1% 字节变异、崩溃现场优先落盘（Crash-First Disk Persistence）与双模式消费验证能力。
- **FR-005**: 必须实现 `SystemDifferentialTests.swift`，调度 `/usr/bin/tar` 与 `/usr/bin/unzip` 实现跨生态双向差分验证。
- **FR-006**: 所有新测试必须完全通过，不侵入已冻结的核心 ZIP 引擎代码，保持零性能倒退。

---

## Success Criteria *(mandatory)*

- **SC-001**: `UUDecoder` 正确解码所有 `.uu` 样本，解码速度 $\ge 100\text{ MB/s}$。
- **SC-002**: `ArchiveGoldenCorpusTests` 100% 通过，覆盖 Zip64、RAR5、7z 畸形与 TAR base256 等关键边界。
- **SC-003**: `ArchiveMutationFuzzTests` 运行 500 次变异迭代保持 100% 进程稳定（零 Crash、零 SIGSEGV）。
- **SC-004**: `SystemDifferentialTests` 在 macOS 14+ 上与系统原生 CLI 实现双向 100% 差分校验通过。
