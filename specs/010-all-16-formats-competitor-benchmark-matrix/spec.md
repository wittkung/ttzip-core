# Feature Specification: Full 16-Format Competitor Benchmark Matrix (010)

**Feature Directory**: `specs/010-all-16-formats-competitor-benchmark-matrix/`  
**Status**: DRAFT  
**Author**: Antigravity CTO & Performance Architect  
**Created**: 2026-08-15  

---

## 1. Executive Summary & Goals

用户要求：
**“所有的其他格式呢？都需要纳入性能测试体系，并且与竞品比较”**

TTZip 支持的全量 16 种归档压缩格式（ZIP, 7Z, TAR.GZ, TAR.ZST, TAR.BZ2, TAR.XZ, TAR, LZIP, LZ4, BROTLI, LRZIP, AAR, SNAPPY, WIM, DMG, ISO 及 RAR/CAB 穿透解压）必须**全部纳入全自动竞品 1v1 性能压测体系**。

每一项格式均与 macOS 平台最强的官方/多核竞品 CLI 进行同台物理压测比对（包括 Apple `ditto`/`zip`/`aa`/`hdiutil`、7-Zip `7zz`、Meta `zstd -T0`、`pigz`/`bsdtar`、`pbzip2`、`pixz`/`xz`、`plzip`、`lz4`、`brotli`、`lrzip`、`wimlib-imagex`）。

---

## 2. User Scenarios & Acceptance Criteria

### User Story 1 (US1): 全 16 种格式全矩阵自动化压测纳入 (All 16 Formats Matrix)
- **As a** 性能工程师与系统用户，
- **I want** 运行 `AllFormatsPkSuiteTests` 或 `ttzip-cli bench_pk` 时能自动覆盖全 16 种格式的压缩与解压，
- **So that** 每一个格式都有详尽的 TTZip 实测吞吐、竞品实测吞吐、胜负战力比与优势数据。

### User Story 2 (US2): 全格式 1v1 竞品对决报表与看板 (Comprehensive Report & Dashboard)
- **As a** 开发者，
- **I want** 压测结束后自动输出包含全 16 种格式的 Markdown 战报与 JSON 数据矩阵，
- **So that** 能一目了然看到 TTZip 在每一个细分格式上的领先优势与优化成果。

### User Story 3 (US3): 全格式零性能倒退审计闭环 (16-Format Regression Audit)
- **As a** 架构师，
- **I want** 全格式基准纳入自动化性能倒退审计（`audit_performance_regression.py`），
- **So that** 任何代码修改均对全 16 种格式进行回归门禁守护。

---

## 3. Functional Requirements (FR)

- **FR-001 (全格式对决测试用例)**:
  - 扩展 `Tests/TTZipTests/AllFormatsPkSuiteTests.swift`，支持包含全 16 种格式（`.zip`, `.sevenZip`, `.tarGz`, `.tarZst`, `.tarBz2`, `.tarXz`, `.tar`, `.lzip`, `.lz4`, `.brotli`, `.lrzip`, `.aar`, `.snappy`, `.wim`, `.dmg`, `.iso`）的自动化竞品 PK。
- **FR-002 (竞品执行器全覆盖与稳健容错)**:
  - 针对每个格式自动探测最佳竞品（如 BZIP2 $\to$ `pbzip2`/`bzip2`, XZ $\to$ `pixz`/`xz`/`7zz`, LZIP $\to$ `plzip`/`lzip`, LZ4 $\to$ `lz4`, BROTLI $\to$ `brotli`, LRZIP $\to$ `lrzip`, AAR $\to$ Apple `aa`, WIM $\to$ `wimlib-imagex`/`7zz`, DMG/ISO $\to$ `hdiutil`/`7zz`）。
  - 若系统未安装某第三方特定 CLI，平滑 fallback 至通用工具（如 7zz）或跳过该竞品并记录清晰日志，杜绝测试崩溃。
- **FR-003 (临时磁盘与生命周期清理)**:
  - 确保全 16 种格式在每次 pass 压测后即时调用 `removeItem` 清理中间镜像与解压目录，杜绝磁盘脏页累积。

---

## 4. Success Criteria (SC)

- **SC-001 (格式全覆盖)**: `AllFormatsPkSuiteTests` 成功覆盖全量格式的压缩与解压 PK。
- **SC-002 (对决数据落盘)**: 自动生成包含全格式对比数据的 `benchmark_report_*.json` 与 `benchmark_report_*.md`。
- **SC-003 (单测与门禁 100% 绿灯)**: `./scripts/run_all_tests.sh` 与 `swift test --filter XCTestPerformanceMeasureTests` 100% 绿灯通过。
