# Quickstart: Comprehensive Corpus Orchestration & Geometric Mean Benchmark Matrix

## Scenario 1: 运行 5-Tier 全量多语料综合基准测试

### Command
```bash
swift test --filter ComprehensiveCorpusBenchmarkPkTests
```

### Expected Output
- 控制台输出 5 大 Tier 分项评测结果及加权几何平均综合得分表。
- 生成综合效能帕累托图表 `pareto_composite_geometric.png`。

### Failure Diagnostic
- 若缺失某项语料，检查 `Tests/TTZipTests/Fixtures/Silesia/` 是否完整，或检查 `CorpusOrchestrator` 路径解析。
