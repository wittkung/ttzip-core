# Quickstart: 7Z Grand Slam Benchmark & Verification

## 1. Run 7Z 1v1 PK Suite
```bash
TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests
```

## 2. Inspect 7Z Leaderboard
```bash
python3 /Users/kevintung/.gemini/antigravity/brain/9148f433-906b-4e79-a8e1-d4f5ea9af6fb/scratch/analyze_new_7z.py
```

## 3. Run Zero-Regression Audit
```bash
python3 scripts/audit_performance_regression.py docs/benchmarks/benchmark_report_2026-08-15_045106.json
```

## 4. Run Performance Gates
```bash
swift test --filter XCTestPerformanceMeasureTests
./scripts/run_all_tests.sh
```
