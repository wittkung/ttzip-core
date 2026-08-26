# Feature Specification: 078-lzfse-dmg-windows-support

**Feature Branch**: `078-lzfse-dmg-windows-support`

**Created**: 2026-08-18

**Status**: Draft

**Input**: User description: "为 Windows 版 TTZip 补齐对 Apple DMG / LZFSE 归档的穿透解压能力，深入剖析现有实现与 apple/lzfse 库机制，制定跨平台静态集成方案。"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Windows 端 Apple DMG (LZFSE 压缩块) 穿透解压与浏览 (Priority: P1)

Windows 用户在 TTZip 中打开或解压由现代 macOS (macOS 10.11+) 生成的 Apple DMG 磁盘映像文件（UDIF 格式，内部包含 LZFSE `0x80000006` 压缩块）。用户能够像操作标准 ZIP / 7Z 一样，快速浏览 DMG 内部的 HFS+ / APFS 分区与文件目录结构，并顺利提取出原始文件，不再出现“格式不受支持”或“未知压缩方法”的错误。

**Why this priority**: Apple DMG 是 macOS 生态最核心的软件分发与磁盘归档格式。现代 DMG 普遍默认采用 LZFSE 块压缩，Windows 平台长期缺乏轻量且高效的穿透解压工具。补齐该能力可彻底打通跨平台 Apple 归档访问体验。

**Independent Test**: 在 Windows 环境下输入包含 LZFSE 压缩块的 `.dmg` 文件，执行解压操作，验证解压出的文件哈希与在 macOS 上原生挂载读取的文件内容 100% 比特精确一致。

**Acceptance Scenarios**:

1. **Given** 一个包含 LZFSE 压缩块（`0x80000006`）的 macOS `.dmg` 文件，**When** 用户在 Windows 端 TTZip 中双击解压，**Then** 系统成功解析 `koly` 尾部签名与 `blkx` 块描述表，将全部数据完整解压至目标目录，退出码为 0。
2. **Given** 一个混合包含 ZLIB、BZIP2、RAW 与 LZFSE 压缩块的复杂 DMG 映像，**When** 用户请求目录列表浏览，**Then** 系统秒级展示分区完整树形结构与文件元数据（文件名、大小、时间戳）。

---

### User Story 2 - 跨平台 `.lzfse` 单文件与流式解压 (Priority: P2)

用户在 Windows 或非 macOS 系统上遇到由 Apple 生态生成的独立 `.lzfse` 压缩文件或日志流时，可以直接使用 TTZip 桌面端或 `ttzip-cli` 进行极速解压与内容还原，无需安装复杂的第三方 Python 脚本或 Apple 专有环境。

**Why this priority**: LZFSE 作为独立压缩格式在 Apple 系统日志、OTA 升级包、系统固件提取物中广泛使用，独立解压能力是完整的 Apple 归档支持链条不可或缺的一环。

**Independent Test**: 使用 `ttzip-cli x sample.lzfse -o out_dir` 解压独立 LZFSE 文件，断言输出文件与源文件校验和完全匹配。

**Acceptance Scenarios**:

1. **Given** 一个经过 LZFSE 压缩的 `.lzfse` 单文件，**When** 用户发起解压指令，**Then** 系统在内存微缓冲流式管道中完成解压，解压吞吐达到 >= 800 MB/s（x86_64 / ARM64）。
2. **Given** 一个包含 LZVN 紧凑块（`dnxv`）的 LZFSE 文件流，**When** 执行流式解压，**Then** 解压器正确识别 LZVN 标记并透明解码，零内存泄漏。

---

### User Story 3 - 静态跨平台 C 绑定与零外部动态库依赖 (Priority: P3)

构建与发布 Windows 版 TTZip 或跨平台 CLI 时，底层 LZFSE 编解码引擎直接以静态 C 代码编译进入 `CTTZipBridge`，在构建产物中不产生对外部 `/usr/lib/liblzfse.dylib` 或系统专有 API 的动态依赖，确保开箱即用与绿色免安装运行。

**Why this priority**: 消除跨平台环境下的动态链接故障，确保在任何 x86_64 或 ARM64 Windows / Linux 环境下均具备确定性行为。

**Independent Test**: 在完全没有安装 macOS 兼容层或额外 DLL 的干净 Windows 沙盒中运行 TTZip，验证 LZFSE / DMG 解压功能正常工作。

**Acceptance Scenarios**:

1. **Given** 干净的 Windows / Linux 构建环境，**When** 执行构建，**Then** `CTTZipBridge` 成功静态编译链接 `apple/lzfse` C99 源码，0 编译警告与 0 动态链接缺失。

---

### Edge Cases

- **损坏或截断的 LZFSE 块**：当 DMG 中的某一 LZFSE 压缩块由于传输错误发生比特翻转或长度不足时，解压器必须返回明确的校验错误码（`TTZIP_ERR_CORRUPT_HEADER` / `TTZIP_ERR_DECOMPRESS_FAILED`），并安全释放所有已分配的 scratch 内存，严禁发生越界读取或段错误。
- **超大 DMG 映像 (>= 100GB)**：处理几十至上百 GB 的大型 DMG 映像时，解压管道必须遵循四大系统工程铁律中的“流式第一性”，单块解码最大内存占用严格限制在单个 chunk 大小（通常 <= 2MB），严禁尝试将整个映像全部读入内存。
- **嵌套 HFS+ / APFS 复合分区**：DMG 内部解压出的 raw 分区映像若为 APFS 或 HFS+，解析器需正确调度文件系统提取逻辑，对 macOS 特有的 AppleDouble (`._*`) 与 `.DS_Store` 按用户过滤策略安全处理。

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 系统 MUST 将 `apple/lzfse` 官方 C99 源码静态集成至 `CTTZipBridge`，彻底替代原先基于 macOS `dlopen("/usr/lib/liblzfse.dylib")` 的动态加载方案。
- **FR-002**: 系统 MUST 在 Windows / macOS / Linux 全平台提供统一的 `ttzip_lzfse_decompress` 和 `ttzip_lzfse_compress` 内存与流式接口。
- **FR-003**: 系统 MUST 支持解析 Apple DMG (UDIF) 磁盘映像中的 `0x80000006` (`BLOCK_LZFSE`) 压缩块类型，并将其正确还原为原始分区扇区数据。
- **FR-004**: 系统 MUST 保持对 DMG 其它已有块类型（`BLOCK_RAW`, `BLOCK_ZERO`, `BLOCK_ZLIB`, `BLOCK_BZIP2`, `BLOCK_LZMA`）的完全兼容。
- **FR-005**: 系统 MUST 遵循流式第一性原则，在 LZFSE 解码热路径中复用预分配的 scratch 缓冲区（`lzfse_decode_scratch_size()`），杜绝热循环中的频繁堆内存分配。
- **FR-006**: 系统 MUST 支持独立 `.lzfse` 单文件的格式识别、校验与穿透解压。
- **FR-007**: 系统 MUST 在 Windows 平台上提供与 macOS 等价的 DMG 目录浏览与文件提取功能。

### Key Entities

- **DMGUDIFDescriptor**: 表示 DMG 文件尾部 512 字节 `koly` trailer 解析后的数据结构，包含 XML plist 偏移量、长度及扇区总数。
- **UDIFChunkBlock**: 表示 DMG 分区映射表（`blkx`）中的单个数据块描述符，包含块类型（如 `0x80000006`）、扇区起始位置、扇区数量、压缩数据偏移量与压缩大小。
- **LZFSEScratchBuffer**: LZFSE 编解码器复用的专用内存刮擦板，固定生命周期内避免堆碎片。

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Windows 环境下对 LZFSE 压缩的 DMG 磁盘映像解压成功率达到 100%（针对有效 DMG 样本）。
- **SC-002**: LZFSE 解压引擎在标准 x86_64 与 ARM64 处理器上的单核物理解压吞吐达到 >= 800 MB/s。
- **SC-003**: 引入静态 `apple/lzfse` 后，C 桥接层二进制体积增长控制在 <= 120 KB。
- **SC-004**: 解压超大 DMG 映像（如 50 GB）时，进程驻留物理内存峰值（RSS）严格稳定在 <= 64 MB。
- **SC-005**: 全量自动化测试套件（包含 525+ 测试用例与新增 DMG/LZFSE 差分回归测试）100% 绿色通过。

---

## Assumptions

- 目标运行平台包含 Windows 10/11 (x64, ARM64) 以及 macOS 14.0+。
- `apple/lzfse` 遵循 BSD-3-Clause 许可证，与 TTZip 的分发渠道和商业规范完全兼容。
- DMG 内部文件系统主要为 APFS、HFS+、FAT32 与 ISO9660，底层文件系统解析由已有的分区读取模块与 libarchive 协同完成。
