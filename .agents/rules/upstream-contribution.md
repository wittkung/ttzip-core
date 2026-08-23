# Upstream Open-Source Contribution Directives for Autonomous Agents

## 1. Zero Blind Submissions
- Never propose or generate an upstream PR patch without executing the automated pre-flight quality gate (`scripts/upstream_audit_gate.py`).
- Every patch touching hot loops or SIMD instructions must be supported by line-by-line assembly disassembly (`otool -tv` or `llvm-objdump -d`) proving zero unwanted stack spills and bound instruction counts.

## 2. Multi-Workload Matrix Non-Regression Mandate
- A microbenchmark speedup on a single isolated length (e.g. 256 bytes) is invalid if any of the 8 standard workloads (`text`, `striped_rgb`, `dna`, `mixed`, `short_match`, `random`, `literals`, `realistic_rgb`) exhibits a statistically significant regression exceeding 0.0\%$.
- All benchmarks must use 5-repetition mirrored cross-over runs to eliminate thermal and DVFS clock frequency drifts.
- Median CV across all points must not exceed .50\%$.

## 3. Communication & Tone Discipline
- All maintainer-facing text must be written in an authentic, humble, and direct engineering tone.
- Strictly eliminate all repetitive AI boilerplate, buzzwords, and self-aggrandizing claims.
- If a maintainer identifies an issue, immediately acknowledge it, isolate the root cause via single-variable ablation tests, and respond with pure verifiable data.
