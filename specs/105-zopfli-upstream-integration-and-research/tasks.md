# Tasks: Google Zopfli 官方上游集成、深度架构解析与极限无损压制

**Feature ID**: `105-zopfli-upstream-integration-and-research`  
**Status**: COMPLETE (Converged & Validated)  

---

## Phase 1: Setup & Upstream Workspace (初始化与上游环境)

- [x] T001 [P] 验证 `Vendor/zopfli-upstream` 目录结构完整性与 Git 状态 in `Vendor/zopfli-upstream/`
- [x] T002 [P] 运行 upstream 官方独立编译与测试 in `Vendor/zopfli-upstream/Makefile`

---

## Phase 2: Foundational (底层 C 静态桥接与符号导出)

- [x] T003 [P] 挂载 Zopfli 核心源文件到 `Sources/CTTZipBridge/zopfli/` in `Sources/CTTZipBridge/zopfli/deflate.c`
- [x] T004 实现 `ttzip_zopfli_compress_block_with_history` 桥接封装 in `Sources/CTTZipBridge/ttzip_zopfli_engine.c`
- [x] T005 导出公共 C 接口原型声明 in `Sources/CTTZipBridge/include/ttzip_zopfli_engine.h`

---

## Phase 3: [US1] 18 核心分块多线程并发编排与 RFC 1951 流式合规

- [x] T006 [P] [US1] 在 `ZipCompressionProfile.swift` 中配置 Level 6 (5 轮) 与 Level 7 (15 轮) 专属 Profile in `Sources/TTZipCore/Zip/ZipCompressionProfile.swift`
- [x] T007 [US1] 在 `ZipExtremeBlockWriter.swift` 中实现基于 18 核 Tile 并发调度并调用 `ttzip_zopfli_compress_block_with_history` in `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift`
- [x] T008 [US1] 为前 $N-1$ 块注入 32KB 跨 Tile 历史字典并生成 `BFINAL=0` 与 `Z_SYNC_FLUSH` 字节对齐标记 in `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift`

---

## Phase 4: [US2] 真实解压与系统原生工具验证

- [x] T009 [P] [US2] 编写 10MB 与 100MB 真实语料下的 Zopfli 并发压缩与解压完整性单元测试 in `Tests/TTZipTests/ZipExtremeBlockWriterTests.swift`
- [x] T010 [US2] 执行 `/usr/bin/unzip -t` 与 `unzip -p` 物理断言零错误通过 in `Tests/TTZipTests/ZipExtremeBlockWriterTests.swift`

---

## Phase 5: [US3] 全量现场实测与帕累托图表渲染

- [x] T011 [US3] 在 `ZipMultiCoreParetoFrontierPkTests.swift` 中运行全量现场实时实测 in `Tests/TTZipTests/ZipMultiCoreParetoFrontierPkTests.swift`
- [x] T012 [US3] 验证生成最新高分辨率帕累托对决图并断言 L6/L7 处于右上角 in `pareto_pk_zip_multicore.png`

---

## Phase 6: [US4] 深度算法解析报告与 Upstream 优化点梳理

- [x] T013 [P] [US4] 撰写 Zopfli 深度架构解析报告与 NEON/SWAR 加速潜力分析 in `docs/research/zopfli_deep_architecture_analysis.md`

---

## Phase 7: Polish & Performance Gate Verification (收敛与门禁验证)

- [x] T014 运行 13 项硬性能门禁测试断言全绿 in `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift`
- [x] T015 执行 `speckit-analyze` 跨工件一致性校验与 Git Commit 同步 in `specs/105-zopfli-upstream-integration-and-research/`
