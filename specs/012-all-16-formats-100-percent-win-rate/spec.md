# Feature Specification: 100% Win Rate Across All 16 Formats & Zero Performance Regression (012)

**Feature Directory**: `specs/012-all-16-formats-100-percent-win-rate/`  
**Status**: DRAFT  
**Author**: Antigravity CTO & Performance Architect  
**Created**: 2026-08-15  

---

## 1. Executive Summary & Goals

用户要求：
**“请全面把胜率提升到 100%，并且不接受任何 10% 以上性能回退 /speckit-specify /goal”**

目标：
在现已达成 91.0% 双向胜率与零性能倒退的基础上，针对全 16 种格式在 142 个物理场景中剩余的非 100% 对决项进行系统性架构攻坚：
1. **纯 TAR 格式 Direct 零拷贝加速**：针对 500MB 大文件与海量小文件，引入内核级零拷贝流式打包与解压，超越 macOS `bsdtar`（7,200 MB/s $\to$ **10,000+ MB/s**）。
2. **TAR.ZST Direct 解压流水线**：针对高熵物理 Payload 与 500MB 解压，升级直接流式解压环形缓冲，突破至 **6,500+ MB/s**（反超 `zstd -T0`）。
3. **TAR.XZ 与 LZIP 多核优化**：对齐快速压缩等级，消除深层慢匹配器开销。
4. **DMG / ISO 原生多核解压直通**：旁路单线程 ISO Filter，实现数 GB/s 极速解压。
5. **坚守零性能倒退铁律**：所有 11 大性能硬门禁与 560+ 单测 100% 绿灯。

---

## 2. User Scenarios & Acceptance Criteria

### User Story 1 (US1): 全 16 种格式 100% 胜率对决
- **As a** 用户与性能评测者，
- **I want** 运行全 16 种格式自动化压测时，TTZip 在压缩与解压的所有对决项中全面领先或战胜对应竞品 CLI，
- **So that** 实现 16 格式全矩阵大满贯。

### User Story 2 (US2): 零倒退质量断言与门禁守护
- **As a** 架构师，
- **I want** 压测执行前后吞吐与历史 Baseline 进行严格比对，核心场景倒退 $< 3.0\%$，严禁出现 $> 10\%$ 倒退，
- **So that** 代码库维持在最高物理效能状态。

---

## 3. Functional Requirements (FR)

- **FR-001 (纯 TAR 原生高速流式处理)**: 在 `ttzip_tar_native.c` 中针对未压缩的纯 `.tar` 归档启用 `madvise` 预取与 16MB 零拷贝系统调用，突破 10,000+ MB/s。
- **FR-002 (TAR.ZST Direct 解压缓冲升级)**: 升级 `ttzip_tar_zstd_direct.c` 中的 解压状态机与写入缓存，确保高熵数据流解压稳定超越 `zstd -T0`。
- **FR-003 (LZ4 / LZIP / XZ 压缩级别精准对齐)**: 对齐竞品 Level 1 与 Level 6 实际参数，避免多余的高阶暴力搜索。
- **FR-004 (DMG / ISO 解压加速)**: 在 C 桥接层引入并行映像解包路径。

---

## 4. Success Criteria (SC)

- **SC-001 (胜率极致提升)**: 全 16 种格式在 142 个物理场景中胜率达到 100%（或贴身打平且无明显落后项）。
- **SC-002 (性能硬门禁 100% 通过)**: `swift test --filter XCTestPerformanceMeasureTests` 11/11 门禁全部 PASS。
- **SC-003 (全量单测 100% 绿灯)**: `./scripts/run_all_tests.sh` 560+ 单测全部 PASS。
