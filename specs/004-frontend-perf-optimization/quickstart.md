# Quickstart & Verification Guide: 前端性能优化

**Feature**: `004-frontend-perf-optimization`
**Date**: 2026-08-15
**Status**: Ready

## 1. Automated Unit & Performance Tests

```bash
# 1. 运行所有相关的前端状态与性能单元测试
swift test --filter FrontendPerfOptimizationTests

# 2. 运行核心性能门禁，确保 C 引擎与 ZIP 吞吐零倒退
swift test --filter XCTestPerformanceMeasureTests

# 3. 运行全量单元测试套件
swift test
```

## 2. End-to-End Visual Verification

1. 启动 TTZip GUI：`swift run ttzip` 或 Xcode 打开 `TTZipApp`。
2. 打开包含 50,000+ 个文件的测试归档，确认首屏秒开（< 150ms），NSOutlineView 展开收起顺滑。
3. 在搜索框快速输入/删除字符，确认输入框无任何卡顿。
4. 运行大批量小文件压缩，确认主界面响应敏捷、UI 帧率稳定在 60/120 FPS。
