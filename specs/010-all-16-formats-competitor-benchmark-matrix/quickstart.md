# Quickstart: 16-Format Competitor Benchmark Execution

```bash
# 1. 执行全 16 种格式竞品对决自动化测试
TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests

# 2. 命令行全格式 1v1 擂台赛
swift run ttzip-cli bench_pk --all-formats

# 3. 运行全套单元测试验证
./scripts/run_all_tests.sh
```
