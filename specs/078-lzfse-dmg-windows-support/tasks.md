# Tasks: 078-lzfse-dmg-windows-support

**Branch**: `078-lzfse-dmg-windows-support` | **Date**: 2026-08-18 | **Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/078-lzfse-dmg-windows-support/spec.md) | **Plan**: [plan.md](file:///Users/kevintung/Documents/dev/TTZip/specs/078-lzfse-dmg-windows-support/plan.md)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: 准备 `apple/lzfse` 官方 C99 源码文件与构建系统目标配置。

- [ ] T001 [P] Create LZFSE in-tree directory structure at `Sources/CTTZipBridge/lzfse/`
- [ ] T002 [P] Embed official `apple/lzfse` C99 header files (`lzfse.h`, `lzfse_internal.h`, `lzfse_tunables.h`, `lzfse_encode_tables.h`, `lzfse_fse.h`, `lzvn_encode_base.h`, `lzvn_decode_base.h`) in `Sources/CTTZipBridge/lzfse/`
- [ ] T003 [P] Embed official `apple/lzfse` C99 source files (`lzfse_encode.c`, `lzfse_encode_base.c`, `lzfse_decode.c`, `lzfse_decode_base.c`, `lzfse_fse.c`, `lzvn_encode_base.c`, `lzvn_decode_base.c`) in `Sources/CTTZipBridge/lzfse/`
- [ ] T004 Update `Package.swift` to add `.headerSearchPath("lzfse")` to `CTTZipBridge` target

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 重构底层 C 桥接层，消除 `dlopen`，建立 Thread-Local Scratch 内存管理与微缓冲流式接口。

**⚠️ CRITICAL**: 必须完成本阶段后方可开始推进上层 User Story 解压与穿透逻辑。

- [ ] T005 Refactor `Sources/CTTZipBridge/CTTZipBridge_LZFSE.c` to link directly against static `lzfse_encode_buffer` and `lzfse_decode_buffer`, completely eliminating `dlopen`/`dlsym`
- [ ] T006 Implement Thread-Local Scratch Arena (`pthread_key_t` / `lzfse_decode_scratch_size()`) in `Sources/CTTZipBridge/CTTZipBridge_LZFSE.c` to guarantee zero-heap-allocation in hot loops
- [ ] T007 [P] Update `Sources/CTTZipBridge/include/CTTZipBridge_LZFSE.h` with block and micro-buffering stream pull interfaces (`ttzip_lzfse_decompress_block`, `ttzip_lzfse_read_ahead`, `ttzip_lzfse_consume`)
- [ ] T008 [P] Implement Swift native adapter `Sources/TTZipCore/Adapters/LzfseCAdapter.swift` conforming to `Sendable` and zero-copy memory pointer protocols

**Checkpoint**: 基础 C 桥接层与线程局部内存池就绪，可无缝支持多线程高并发解压。

---

## Phase 3: User Story 1 - Apple DMG (LZFSE 0x80000006/0x80000007 块) 穿透解压与浏览 (Priority: P1) 🎯 MVP

**Goal**: 使得 Windows 版与跨平台版 TTZip 能够完整解析 Apple DMG (UDIF) 尾部 `koly` trailer、`blkx` mish 块表，并将 LZFSE 压缩块（`0x80000006`/`0x80000007`）秒级还原，穿透浏览/提取 APFS 与 HFS+ 分区中的文件。

**Independent Test**: 在非 macOS 模拟环境或 Windows 目标下，输入包含 LZFSE 块的 APFS DMG 镜像，执行穿透解压并校验解压出的文件哈希 100% 匹配。

### Tests for User Story 1
- [ ] T009 [P] [US1] Create unit and regression tests for UDIF trailer parsing and LZFSE chunk decoding in `Tests/TTZipTests/DMGLZFSEExtractionTests.swift`

### Implementation for User Story 1
- [ ] T010 [P] [US1] Implement C-level UDIF container demuxer header `Sources/CTTZipBridge/include/ttzip_dmg_demux.h` (koly trailer, blkx mish table structures)
- [ ] T011 [US1] Implement C-level UDIF container demuxer `Sources/CTTZipBridge/ttzip_dmg_demux.c` with Big-Endian safe parsing and LZFSE / ZLIB / RAW chunk iteration
- [ ] T012 [P] [US1] Implement virtual sector stream bridge `Sources/TTZipCore/SevenZip/DMGVirtualStreamAdapter.swift` to assemble decoded DMG chunks into contiguous partition streams
- [ ] T013 [US1] Hook DMG LZFSE fast-path extraction into `Sources/TTZipCore/ArchiveExtractor+Dispatch.swift` to route `.dmg` files to the native UDIF + LZFSE engine
- [ ] T014 [US1] Add DMG partition inspection and folder tree extraction logic in `Sources/TTZipCore/ArchiveExtractor.swift`
- [ ] T015 [US1] Verify DMG LZFSE schema compliance against `specs/078-lzfse-dmg-windows-support/contracts/dmg-udif-schema.json`

**Checkpoint**: User Story 1 完成，Windows 端可成功穿透解压由现代 macOS 导出的 DMG 磁盘映像。

---

## Phase 4: User Story 2 - 跨平台 `.lzfse` 单文件与微缓冲流式拉取解压 (Priority: P2)

**Goal**: 支持对独立 `.lzfse` 压缩文件与流式日志进行极速解压，内存物理常驻（RSS）严格控制在 $\le 64\text{MB}$。

**Independent Test**: 运行 `swift test --filter DMGLZFSEStreamingMemoryGateTests` 验证 50GB 虚拟流式大文件解压时 RSS $\le 64\text{MB}$。

### Tests for User Story 2
- [ ] T016 [P] [US2] Create large streaming memory gate test in `Tests/TTZipTests/DMGLZFSEStreamingMemoryGateTests.swift`

### Implementation for User Story 2
- [ ] T017 [US2] Implement micro-buffering pull pipeline in `Sources/CTTZipBridge/CTTZipBridge_LZFSE.c` replacing `mmap(total_size)` + `malloc(total_size * 8)`
- [ ] T018 [P] [US2] Add single-file `.lzfse` file format signature detection in `Sources/TTZipCore/Standards/ArchiveMagicSignatureScanner.swift`
- [ ] T019 [US2] Add `.lzfse` single-file extraction routing in `Sources/TTZipCore/ArchiveExtractor.swift` and `Sources/TTZipCore/ArchiveExtractor+Dispatch.swift`
- [ ] T020 [US2] Verify LZFSE codec contract compliance against `specs/078-lzfse-dmg-windows-support/contracts/lzfse-codec-contract.json`

**Checkpoint**: User Story 2 完成，独立 `.lzfse` 文件支持流式秒级解压，大文件零 OOM。

---

## Phase 5: User Story 3 - 静态跨平台 C 绑定与零外部动态库依赖 (Priority: P3)

**Goal**: 确保在干净的 Windows / Linux / macOS 构建环境下，编译产物具备 100% 静态内联链接，零外部动态库丢失风险。

**Independent Test**: 执行 `swift test --filter AccelerationInfrastructureTests/testLZFSEStreamRoundTrip` 验证端到端往返编解码与可用性。

### Implementation for User Story 3
- [ ] T021 [US3] Update `Tests/TTZipTests/AccelerationInfrastructureTests.swift` to verify static LZFSE roundtrip compression and decompression without dynamic loading
- [ ] T022 [P] [US3] Validate zero-warning compilation on macOS Release and MAS sandbox builds (`swift build -c release` & `swift build -c release -Xswiftc -DMAS_BUILD`)

**Checkpoint**: 全平台纯静态编译验证完成，消除任何外部 dylib 隐式依赖。

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: 性能门禁压测、四大系统工程铁律合规审计与基准测试差分比对。

- [ ] T023 [P] Execute `swift test --filter XCTestPerformanceMeasureTests` and verify all performance floors are met
- [ ] T024 [P] Execute full regression test suite `swift test` (ensure 525+ tests pass with 0 regressions)
- [ ] T025 Run quickstart validation guide in `specs/078-lzfse-dmg-windows-support/quickstart.md`
- [ ] T026 Update `ARCHITECTURE.md` to document the new LZFSE static engine and DMG UDIF demuxer architecture

---

## Dependencies & Execution Order

### Phase Dependencies
- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 — BLOCKS all user stories
- **User Story 1 (Phase 3)**: Depends on Phase 2 — MVP target
- **User Story 2 (Phase 4)**: Depends on Phase 2 — can run in parallel with or after US1
- **User Story 3 (Phase 5)**: Depends on Phase 2, 3, 4
- **Polish (Phase 6)**: Depends on all user stories being complete

### Parallel Opportunities
- In Phase 1: T001, T002, T003 can run in parallel.
- In Phase 2: T007 and T008 can run in parallel after T005/T006.
- In Phase 3: T009, T010, T012 can run in parallel.
- In Phase 4: T016 and T018 can run in parallel.
- In Phase 6: T023 and T024 can run in parallel.

---

## Implementation Strategy (MVP First)

1. **Step 1**: Complete Phase 1 (Setup) & Phase 2 (Foundational) → CTTZipBridge 静态内嵌 LZFSE 源码并消除 `dlopen`。
2. **Step 2**: Complete Phase 3 (User Story 1) → 实现 UDIF demuxer 并挂载 LZFSE 解码器，实现 Windows 端 DMG 穿透解压 (MVP)。
3. **Step 3**: Complete Phase 4 (User Story 2) & Phase 5 (User Story 3) → 流式拉取微缓冲与跨平台构建验证。
4. **Step 4**: Complete Phase 6 (Polish) → 跑通全量 525+ 单元测试与性能门禁。
