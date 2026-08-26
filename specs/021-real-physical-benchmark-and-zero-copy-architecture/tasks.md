# Tasks: 021-real-physical-benchmark-and-zero-copy-architecture

**Input**: User stories from `spec.md`, architecture from `plan.md`, data model from `data-model.md`, schemas from `contracts/`
**Feature Branch**: `021-real-physical-benchmark-and-zero-copy-architecture`

---

## Phase 1: Setup & Grounded Analysis

**Purpose**: 初始化与基准环境校验

- [x] T001 校验 Feature 分支与编译环境 in `Package.swift`
- [x] T002 检查并加载全格式性能审计报告基线 in `docs/benchmarks/latest_regression_audit.md`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 核心接口与数据结构基础设施

- [x] T003 [P] 校验与对齐数据模型实体定义 in `specs/021-real-physical-benchmark-and-zero-copy-architecture/data-model.md`
- [x] T004 [P] 校验 JSON Schema 契约完备性 in `specs/021-real-physical-benchmark-and-zero-copy-architecture/contracts/`
- [x] T005 在 `Sources/TTZipCore/ArchiveCompressionTypes.swift` 中完善 `enableZeroCopy: Bool = false` 基础字段定义

---

## Phase 3: User Story 1 - 真实物理 I/O 基准度量与 28 项倒退攻坚 (Priority: P1) 🎯 MVP

**Goal**: 恢复 ZIP 高熵解压、TAR.ZST、7Z 等核心格式在纯真实物理 I/O 下的最高历史吞吐，消灭全部 28 项 >10% 严重倒退

**Independent Test**:
- 运行 `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests`
- 运行 `python3 scripts/audit_performance_regression.py` 验证严重倒退项清零

### Implementation for User Story 1

- [x] T006 [US1] 修复 `Sources/TTZipCore/TemplateMethod/ZipArchiveEngineTemplate.swift` 中 ZIP 解压直通原生 C 引擎 `ttzip_extract_zip_c_parallel` Fast-Path
- [x] T007 [P] [US1] 修复 `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c` 中高熵物理 Payload 解压短路 Bug 与 USTAR 流水线
- [x] T008 [P] [US1] 优化 `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c` 与 `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c` 中 7Z Level 1 与 Level 5 编码分块
- [x] T009 [P] [US1] 优化 `Sources/CTTZipBridge/CTTZipExtract.c` 中小文件多线程并发文件创建与落盘
- [x] T010 [US1] 运行 `swift test -c release --filter XCTestPerformanceMeasureTests` 验证 11 项物理硬门禁全部绿灯

---

## Phase 4: User Story 2 - APFS 零拷贝技术架构完整实现 (Priority: P1)

**Goal**: 生产环境实现 APFS 写时复制（`enableZeroCopy: true`）秒级打包，且与基准测试严格解耦

**Independent Test**:
- 运行 APFS 零拷贝专属测试用例验证可逆性与克隆准确性

### Implementation for User Story 2

- [x] T011 [US2] 在 `Sources/TTZipCore/Zip/ZipStoreStreamWriter.swift` 中完善 `createStoreArchive` 的 `enableZeroCopy` 参数分支与 `ttzip_apfs_clone_range` 协同
- [x] T012 [P] [US2] 确保 `Sources/TTZipCore/TemplateMethod/ZipArchiveEngineTemplate.swift` 与 `Sources/TTZipCore/ArchiveWriter.swift` 默认设置 `enableZeroCopy: false`
- [x] T013 [US2] 编写与运行 APFS 零拷贝专项单测 in `Tests/TTZipTests/ZipStoreZeroCopyTests.swift`

---

## Phase 5: User Story 3 - 解析器字节对齐健壮性与全量单测全绿闭环 (Priority: P2)

**Goal**: 彻底消除 `CTTZipParser.c` 逆向扫描中的字节对齐缺陷，保证 591+ 单测 100% 绿灯

**Independent Test**:
- 运行 `swift test --filter ArchiveSpecIntegrityTests` 与全量 `swift test`

### Implementation for User Story 3

- [x] T014 [US3] 修复 `Sources/CTTZipBridge/CTTZipParser.c` 中的 `ttzip_find_eocd` 逆向单字节步进扫描逻辑
- [x] T015 [US3] 运行 `swift test` 验证全量 591+ 单元测试 0 失败

---

## Phase 6: User Story 4 - 全格式零倒退门禁全覆盖与自动化审计防护网 (Priority: P1)

**Goal**: 扩展门禁覆盖全部 16 种格式的核心场景，并集成自动化审计脚本 `--strict` 门禁

**Independent Test**:
- 运行 `python3 scripts/audit_performance_regression.py --strict`

### Implementation for User Story 4

- [x] T016 [US4] 扩展 `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift` 中的性能测试用例覆盖全部 16 种格式
- [x] T017 [P] [US4] 在 `scripts/run_all_tests.sh` 中集成 `python3 scripts/audit_performance_regression.py` 自动化零倒退卡点
- [x] T018 [US4] 同步更新 `GEMINI.md` 中的全格式性能门禁与审查纪律

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: 最终收敛与端到端交付验证

- [x] T019 [P] 执行契约合规性核对 in `specs/021-real-physical-benchmark-and-zero-copy-architecture/contracts/`
- [x] T020 执行端到端快速验证 in `specs/021-real-physical-benchmark-and-zero-copy-architecture/quickstart.md`
- [x] T021 执行 `python3 scripts/audit_performance_regression.py` 输出最终性能比对报告

---

## Dependencies & Execution Order

### Phase Dependencies
- **Setup (Phase 1)**: 无依赖，立即开始
- **Foundational (Phase 2)**: 依赖 Setup 完成，阻塞所有 User Stories
- **User Story 1 (Phase 3)**: 依赖 Foundational 完成，为 MVP 核心
- **User Story 2 (Phase 4)**: 依赖 Foundational 完成，可与 US1 并行开发
- **User Story 3 (Phase 5)**: 依赖 Foundational 完成，保证规范合规
- **User Story 4 (Phase 6)**: 依赖 US1、US2、US3 完成后执行全量门禁固化
- **Polish (Phase 7)**: 依赖所有 User Stories 完成

### Parallel Execution Opportunities
- **Foundational**: T003 与 T004 可并发进行
- **User Story 1**: T007、T008、T009 分属不同底层 C 文件，可并发执行
- **User Story 4**: T017 可与 T016 并发执行
