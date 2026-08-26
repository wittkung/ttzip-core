# Tasks: 7z 全链路原生压缩流算法全景调研与自主无依赖引擎演进

**Input**: Design documents from `specs/108-7z-native-compression-pipeline/`  
**Prerequisites**: `plan.md` (required), `spec.md` (required), `research.md`, `data-model.md`, `contracts/`  
**Organization**: Tasks are grouped by user story to enable independent implementation and testing.

---

## Phase 1: Setup & Foundational Infrastructure

**Purpose**: 确立架构审计上下文与纯自研 C 桥接层基础头文件契约。

- [ ] T001 [P] 建立 7z 原生引擎契约定义与数据模型映射在 `Sources/CTTZipBridge/include/ttzip_lzma2_dec_native.h`
- [ ] T002 [P] 确立极速 Double-Fast 编码器状态机定义在 `Sources/CTTZipBridge/include/ttzip_lzma2_fast_encoder.h`
- [ ] T003 [P] 确立自研最优解析器与 Radix 匹配查找器接口在 `Sources/CTTZipBridge/include/ttzip_fl2_lzma2.h`

---

## Phase 2: User Story 1 - 7z 压缩流底层实现全景审计与依赖透视 (Priority: P1) 🎯 MVP

**Goal**: 完成对 `Sources/TTZipCore/SevenZip/` 与 `Sources/CTTZipBridge/` 中全部 7z 相关源文件的地毯式调用链审计，输出完整的白皮书工件。  
**Independent Test**: 产出包含完整文件、行号与外部库分类的架构审计文档 `docs/architecture/7z_compression_stream_comprehensive_audit.md`。

- [x] T004 [P] [US1] 审计 Swift 调度层与 C 适配器调用链在 `Sources/TTZipCore/SevenZip/SevenZipEngine.swift` 与 `Sources/TTZipCore/Adapters/SevenZipCAdapter.swift`
- [x] T005 [P] [US1] 审计 C 容器层与 Store/AES/KDF/BCJ 核心实现归属在 `Sources/CTTZipBridge/CTTZipBridge_7z.c`, `CTTZipBridge_7zStore.c`, `ttzip_7z_kdf_arm64.c`
- [x] T006 [P] [US1] 审计 LZMA2 编码与解码外部库调用栈在 `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c`, `ttzip_lzma2_dec_native.c`, `ttzip_fl2_bridge.c`
- [x] T007 [US1] 汇编并产出 7z 全链路压缩流算法全景架构白皮书在 `docs/architecture/7z_compression_stream_comprehensive_audit.md`

**Checkpoint**: 7z 底层全景资产与外部依赖清单 100% 物理查实并落盘。

---

## Phase 3: User Story 2 - ZIP 引擎底层优秀架构向 7z 迁移与复用设计 (Priority: P1)

**Goal**: 提取 ZIP 模块已验证成熟的 SWAR/NEON 匹配长度探测器、APFS 预分配与无锁分块多核调度范式，建立 7z 迁移复用契约。  
**Independent Test**: 验证 `ttzip_hybrid_match_len_neon` 与 `ttzip_double_fast_t` 在 7z 匹配探测中的无缝接入。

- [ ] T008 [P] [US2] 验证并适配 SWAR+NEON 混合匹配长度计算算子在 `Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c`
- [ ] T009 [P] [US2] 适配 APFS 磁盘空间预分配与直接 I/O 模型在 `Sources/CTTZipBridge/CTTZipBridge_7zStore.c`
- [ ] T010 [US2] 统一单文件 mmap 零拷贝与多核 Pack Arena 分块调度在 `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c`

**Checkpoint**: ZIP 底层 5 大核心基础设施向 7z 迁移规范完成对接。

---

## Phase 4: User Story 3 - 纯自研 0-外部依赖 7z/LZMA2 引擎全套架构规范 (Priority: P2)

**Goal**: 构建完全脱离 `liblzma.a` 与 `fast-lzma2` 的自研 LZMA2 解码器与 Double-Fast 极速编码器架构。  
**Independent Test**: 编译并运行纯自研 Range Decoder 与 Fast Encoder 单元测试。

- [ ] T011 [P] [US3] 实现 ARM64 CSEL 无分支 Range Decoder 与 Direct Linear Slicing 字典窗口在 `Sources/CTTZipBridge/ttzip_lzma2_dec_native.c`
- [ ] T012 [P] [US3] 完善 Double-Fast (DF-4/8) 512KB L2 缓存表与 1-Step Lookahead 极速编码器在 `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c`
- [ ] T013 [US3] 重构多核分发入口彻底移除 `lzma_raw_buffer_encode` 与 `lzma_raw_decoder` 调用在 `Sources/CTTZipBridge/ttzip_fl2_bridge.c`

**Checkpoint**: 7z 热路径外部静态库依赖全部替换为自研原生 C 实现。

---

## Phase 5: User Story 4 - 性能基准、门禁与回归验证 (Priority: P3)

**Goal**: 运行全矩阵回归与硬性能门禁测试，验证 7z 极速压缩（$\ge 3,800\text{ MB/s}$）与解压（$\ge 7,500\text{ MB/s}$）性能达标，且 525+ 测试全绿。  
**Independent Test**: 执行 `swift test` 与 `swift test --filter XCTestPerformanceMeasureTests` 确认 100% 通过。

- [ ] T014 [P] [US4] 运行 7z 格式基础与加密解密单元测试套件在 `Tests/TTZipTests/SevenZipTests.swift`
- [ ] T015 [US4] 运行硬性能门禁测试验证全矩阵零倒退在 `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift`
- [ ] T016 [US4] 验证系统 `7zz` 双向差分预言机测试在 `Sources/TTZipCLI/`

---

## Dependencies & Execution Order

- **Phase 1 (Setup)**: T001 ~ T003 可并行执行 `[P]`。
- **Phase 2 (US1 审计)**: T004, T005, T006 可并行执行 `[P]`，汇聚于 T007。
- **Phase 3 (US2 ZIP 复用)**: T008, T009 可并行执行 `[P]`，依赖 Phase 1 完成。
- **Phase 4 (US3 纯自研引擎)**: T011, T012 可并行执行 `[P]`，汇聚于 T013。
- **Phase 5 (US4 门禁验证)**: T014, T015, T016 依次运行验证全量指标。
