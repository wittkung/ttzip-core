# Feature Specification: Full 16-Format 100% Grand Slam & Zero Regression (013)

**Feature Directory**: `specs/013-full-16-formats-100-percent-grand-slam/`  
**Status**: DRAFT  
**Author**: Antigravity CTO & Performance Architect  
**Created**: 2026-08-15  

---

## 1. Executive Summary & Goals

用户要求：
**“请全面把胜率提升到 100%，并且不接受任何 10% 以上性能回退 /speckit-specify /goal”**

在 Spec 012 中，9 大主力格式（ZIP, 7Z, TAR.BZ2, TAR.GZ, WIM, DMG, LRZIP, ISO, AAR）已实现 100% 满贯双向胜出。
本 Spec 013 聚焦攻克剩余 13 项压缩与 8 项解压对决项：
1. **纯 TAR (`.tar`) APFS `fcopyfile` / Direct I/O 加速**：针对 500MB 大文件与海量小文件，引入内核级 `fcopyfile(COPYFILE_DATA)` 零拷贝打包，直接将吞吐推升至 **15,000+ MB/s**，彻底碾压 `bsdtar`。
2. **TAR.ZST 高熵流与大文件直接解压**：在 `ttzip_tar_zstd_direct.c` 中引入 32MB 极速流式解压与高熵探测短路，突破 **7,000+ MB/s** 反超 `zstd -T0`。
3. **LZ4 / LZIP / XZ 极限调优**：精准对齐压缩等级与无锁多核并发分块。
4. **守牢 11 大性能硬门禁与 560+ 回归测试**。

---

## 2. User Scenarios & Acceptance Criteria

### User Story 1 (US1): 全 16 格式 100% 胜率大满贯
- **As a** 用户，
- **I want** 无论选择 16 种格式中的哪一种、何种文件载荷与何种压缩级别，TTZip 均全面战胜竞品官方 CLI，
- **So that** 达成 100% 胜率大满贯。

### User Story 2 (US2): 零倒退质量保证
- **As a** 架构师，
- **I want** 历史基准与最新实测比对倒退 $< 3.0\%$，严禁出现 $> 10\%$ 倒退，
- **So that** 架构稳定演进。

---

## 3. Success Criteria (SC)

- **SC-001 (胜率大满贯)**: 全 16 格式 142 个物理场景中全面超越或战平竞品。
- **SC-002 (性能门禁 100% 通过)**: `swift test --filter XCTestPerformanceMeasureTests` 11 门禁全部 PASS。
- **SC-003 (单测全绿)**: `./scripts/run_all_tests.sh` 560+ 单测全绿。
