# Quickstart Validation: 7Z 500MB Final Conquest

```bash
# 执行基准压测
TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests

# 运行 7z 战况排行榜分析
python3 /Users/kevintung/.gemini/antigravity/brain/9148f433-906b-4e79-a8e1-d4f5ea9af6fb/scratch/analyze_new_7z.py

# 验证零倒退审计
python3 scripts/audit_performance_regression.py docs/benchmarks/benchmark_report_2026-08-15_050902.json
```
