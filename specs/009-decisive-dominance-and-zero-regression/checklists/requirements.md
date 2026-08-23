# Feature 009: Decisive Dominance and Zero Regression Requirements Checklist

## User Scenario & Acceptance Verification
- [ ] 7Z 格式全量 32 项对决中，500MB 大文件与 100MB 高熵流实现显著超越（>= 1.05x），零险胜、零打平、零落后。
- [ ] TAR.ZST 格式解压端与高熵压缩端突破 libarchive 瓶颈，反超 Meta `zstd -T0` CLI。
- [ ] 运行全量性能回归比对（`python3 scripts/audit_performance_regression.py`），回落 > 10% 项数为 0。
- [ ] 11 大性能硬门禁（`swift test --filter XCTestPerformanceMeasureTests`）100% 绿灯。
- [ ] 全套单元测试（`./scripts/run_all_tests.sh`）560+ 单测 100% 绿灯。
