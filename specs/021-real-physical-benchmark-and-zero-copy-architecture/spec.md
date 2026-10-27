# Feature Specification: 021-real-physical-benchmark-and-zero-copy-architecture

**Feature Branch**: `021-real-physical-benchmark-and-zero-copy-architecture`

**Created**: 2026-08-15

**Status**: Draft

**Input**: User description: "APFS 零拷贝技术需要实现，但测试中不使用，也不计入性能。完整梳理 /speckit-specify"

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 纯物理真实 I/O 基准度量与门禁解耦 (Priority: P1)

作为开发与质量保证工程师，在运行单元测试、11 项性能硬门禁测试 (`XCTestPerformanceMeasureTests`) 以及全竞品横向 PK 评测 (`AllFormatsPkSuiteTests`) 时，系统必须恒定在**真实物理 I/O**模式下执行（即真实执行内存映射读取、16 核并发 NEON CRC32 校验、以及实际落盘写入存储介质）。基准测试与自动化回归审计必须度量真实的编解码与物理存储吞吐，绝对不使用 APFS 零拷贝（Extent 共享）等文件系统元数据捷径来替代真实物理写盘，杜绝任何因作弊或测试环境缓存抖动产生的虚高跑分污染。

**Why this priority**: 性能评测的根本价值在于衡量真实算法与硬件架构的物理极限吞吐。若基准测试混入零拷贝，会导致跑分失真并使后续回归审计陷入死循环。

**Independent Test**:
- 运行 `swift test --filter XCTestPerformanceMeasureTests`，验证 ZIP Store 模式在执行真实 50MB 物理落盘与 NEON CRC32 计算时，实测吞吐稳定在物理真实区间（$\ge 5,000\text{ MB/s}$），且所有 11 项硬门禁 100% 绿灯通过。

**Acceptance Scenarios**:
1. **Given** 运行 11 项性能硬门禁测试，**When** 执行 `testZipStore_HugeFile_XCTestMeasureMetrics`，**Then** 系统执行真实文件读取、NEON CRC32 计算与 `pwrite` 物理落盘，吞吐量达到真实物理硬件上限（Debug $\ge 4,500\text{ MB/s}$，Release $\ge 5,000\text{ MB/s}$），断言成功。
2. **Given** 运行自动化性能审计脚本 `audit_performance_regression.py`，**When** 比对当前跑分与历史物理基准，**Then** 比对数据完全建立在真实物理 I/O 基础之上，无因零拷贝失真产生的虚假倒退阻断。

---

### User Story 2 - APFS 零拷贝技术架构完整实现 (Priority: P1)

作为 macOS 原生归档工具的终端用户与系统调用方，在生产环境使用 Store 模式归档或文件打包时，系统必须提供高效的 APFS 写时复制（Copy-on-Write / Extent-level Clone）零拷贝技术实现。通过底层 `ttzip_apfs_clone_range` C 语言系统调用绑定与 Swift 参数接口（`enableZeroCopy: Bool`），在支持 APFS 卷的场景下实现毫秒级瞬间打包，并在非 APFS 卷或克隆失败时无缝降级至多核并发直接 I/O 物理写盘。

**Why this priority**: APFS 零拷贝是 macOS 平台的原生核心技术优势，必须在引擎中具备完整的工业级实现供实际生产使用，但其调用通道必须与基准测试解耦受控。

**Independent Test**:
- 编写专项功能测试用例，显式传入 `enableZeroCopy = true`，在 APFS 临时目录下验证克隆打包产物的解压可逆性、CRC32 校验和与文件一致性。

**Acceptance Scenarios**:
1. **Given** 一个位于 APFS 格式卷上的 100MB 输入文件，**When** 调用 `ZipStoreStreamWriter.createStoreArchive(..., enableZeroCopy: true)`，**Then** 归档成功生成，CRC32 校验码与源文件完全一致，且解压验证无误。
2. **Given** 在基准测试或默认归档流程中，**When** 未显式指定开启零拷贝（`enableZeroCopy: false`），**Then** 系统走多核并发页对齐直接物理写盘通道，确保数据物理落盘。

---

### User Story 3 - 解析器字节对齐健壮性与全量单测全绿闭环 (Priority: P2)

作为 TTZip 核心解析引擎的维护者，系统底层 C 语言解析器（[`CTTZipParser.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipParser.c)）必须具备防御性、类型安全且字节对齐无关的逆向扫描能力。能够准确无误地定位任意字节偏移处的 EOCD（0x06054b50）与 CDFH 目录项，彻底消除由于不当向量跳步引起的扫描遗漏缺陷，确保全量 591+ 项单元测试 100% 通过。

**Why this priority**: 规范完整性与解析健壮性是归档工具的生命线，任何解析器遗漏都会破坏 ZIP 规范校验与解压能力。

**Independent Test**:
- 运行 `swift test --filter ArchiveSpecIntegrityTests`，验证 EOCD 逆向探测与 CDFH 条目解析全部通过。

**Acceptance Scenarios**:
1. **Given** 一个小体积或任意字节对齐的 ZIP 归档文件，**When** 调用 `ttzip_find_eocd` 探测末端记录，**Then** 正确返回 `true` 并提取准确的 `total_entries` 与 `cd_offset`。
2. **Given** 运行全量测试套件 `swift test`，**When** 执行全部 591 个测试用例，**Then** 0 failures, 100% 绿灯。

---

### Edge Cases

- 当输入文件位于不支持 APFS Extent Clone 的外部驱动器（如 exFAT、FAT32 或 SMB 网络共享）时，`ttzip_apfs_clone_range` 返回非零错误码，系统必须即时、平滑降级至分块 `pread`/`pwrite` 物理流式写入，不得崩溃或产生空文件。
- 当 ZIP 归档尾部包含变长注释（ZIP Comment，最大 65,535 字节）导致 EOCD 处于非固定偏移时，逆向扫描器必须能够遍历搜索回溯窗口并在微秒级时间内准确命中签名。

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 系统必须在 `ZipStoreStreamWriter` 与底层 C 桥接层中保留完整的 APFS 零拷贝（`ttzip_apfs_clone_range`）实现，并通过参数 `enableZeroCopy: Bool` 显式受控。
- **FR-002**: 系统必须确保在所有性能度量测试（`XCTestPerformanceMeasureTests`）与基准测试（`CompetitorBenchmarkRunner`）中，默认将 `enableZeroCopy` 设为 `false`，仅度量纯物理 I/O 与 NEON CRC32 吞吐。
- **FR-003**: 系统必须在 `XCTestPerformanceMeasureTests.swift` 与 `GEMINI.md` 中将 ZIP Store 门禁阈值校准为真实物理 I/O 底线（Debug 模式 $\ge 4,500\text{ MB/s}$，Release 模式 $\ge 5,000\text{ MB/s}$），确保门禁在真实物理写盘下可稳定复现且具备严密防护性。
- **FR-004**: 系统必须在 `CTTZipParser.c` 中修复 `ttzip_find_eocd` 的逆向字节扫描逻辑，支持在任意字节边界准确定位 EOCD 标头，保证小文件与未对齐 ZIP 的解析防御性。
- **FR-005**: 系统必须通过全量单元测试（`swift test`，591+ tests 100% PASS）与 11 项性能硬门禁（`swift test -c release --filter XCTestPerformanceMeasureTests` 100% PASS）。

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 全量单元测试套件 `swift test` 执行 591+ 个测试用例，失败项严格为 **0**（100% 绿灯通过）。
- **SC-002**: Release 模式 11 项性能硬门禁测试 `swift test -c release --filter XCTestPerformanceMeasureTests` 全部通过，ZIP Store 真实物理写盘吞吐达 $\ge 5,000\text{ MB/s}$。
- **SC-003**: 专用 APFS 零拷贝测试用例验证成功，且与纯物理 I/O 基准测试实现 100% 逻辑解耦与隔离。

---

## Assumptions

- 运行测试的硬件环境为搭载 Apple Silicon（M 系列芯片）的 macOS 14.0+ 设备。
- 零拷贝技术仅对源文件与目标归档处于同一 APFS 文件系统卷有效，跨卷操作自动透明降级。
- 门禁测试以纯真实物理编解码和直接写盘为准，不计入任何文件系统层面的零拷贝加成。
