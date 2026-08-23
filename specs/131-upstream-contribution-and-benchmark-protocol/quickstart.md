# Quickstart & Validation Guide: Feature 131

## Verification Scenarios

### Scenario 1: Pre-Flight Compiler Flag Audit
- **Command**: `python3 scripts/upstream_crossover_bench.py --verify-flags --baseline build_develop --candidate build`
- **Expected Output**: `[PASS] 100% Compiler flag parity verified between baseline and candidate.`
- **Failure Diagnostic**: If flags mismatch, check `CMakeCache.txt` for `CMAKE_BUILD_TYPE` or `WITH_NATIVE_INSTRUCTIONS`.

### Scenario 2: Dual Cross-Over Benchmark Execution
- **Command**: `python3 scripts/upstream_crossover_bench.py --runs 5 --baseline-bin build_develop/test/benchmarks/benchmark_zlib --candidate-bin build/test/benchmarks/benchmark_zlib`
- **Expected Output**: Generates `crossover_results.json` with Order A, Order B, and Cross-Over Mean.
- **Failure Diagnostic**: Ensure machine is connected to AC power and background CPU load is <5%.

### Scenario 3: Zero-Hallucination Dynamic Report Generation
- **Command**: `python3 scripts/upstream_report_gen.py --json crossover_results.json --output-pr pr_desc.md --output-comment comment.md`
- **Expected Output**: Generates `pr_desc.md` and `comment.md` with all dynamic fields populated from JSON.
- **Failure Diagnostic**: Verify JSON schema compliance against `contracts/benchmark-result.schema.json`.
