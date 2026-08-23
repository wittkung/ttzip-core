# Quickstart & Verification Guide: Upstream Contribution Methodology, Lessons Learned, and Engineering Governance

**Feature Directory**: `specs/133-upstream-contribution-lessons-and-governance`  
**Target Subject**: 自动化审计门禁执行、宪章合规性扫描与教学知识树验证  

---

## 1. Scenario 1: Automated Upstream Pre-Flight Audit Gate Execution (US1)

### Command
```bash
python3 scripts/upstream_audit_gate.py --worktree Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256 --baseline develop --candidate feat-arm64-swar-compare256 --target compare256_neon
```

### Expected Output
```text
======================================================================
⚡️ Upstream Pre-Flight Quality Gate (Hardware & Statistical Rigor)
======================================================================
[Stage 1/5] Compiler Flag Parity Audit .............. [PASS] (Identical -O3, -DNDEBUG)
[Stage 2/5] Dual Build & Zero-Warning Audit ......... [PASS] (CMake + Autotools 0 warnings)
[Stage 3/5] Assembly Disassembly Audit .............. [PASS] (149 instructions, 0 stack spills)
[Stage 4/5] Statistical CV Analysis ................. [PASS] (Median CV: 1.05% <= 1.50%)
[Stage 5/5] Multi-Workload Single-Point Gate ........ [PASS] (50/50 points, Max Regression: 0.0%)

✅ Upstream Pre-Flight Gate PASSED!
Artifact generated: /tmp/upstream_audit_report.json
```

### Failure Diagnostic
- **Issue: Stage 1 Flag Mismatch**: Baseline and Candidate CMakeCache.txt differ in optimization flags. Check .
- **Issue: Stage 4 High Variance (CV > 1.5%)**: Thermal throttling or CPU scheduler interference. Allow 60s cooldown and re-run with background applications closed.
- **Issue: Stage 5 Single-Point Regression**: A specific workload regressed by > 2.0%. Inspect the offending workload in  and perform scalar early-exit refactoring.

---

## 2. Scenario 2: Engineering Constitution Invariants Verification (US2)

### Command
```bash
python3 -c "
with open('.specify/memory/constitution.md') as f:
    text = f.read()
assert '## 6. Upstream Open-Source Contribution & Hardware Grounding Protocol' in text, 'Constitution missing Section 6'
assert 'Invariant 1: Hardware Grounding' in text
assert 'Invariant 2: Multi-Workload Zero Regression' in text
assert 'Invariant 3: Single-Variable Ablation Testing' in text
assert 'Invariant 4: Maintainer Attention Reverence' in text
assert 'Invariant 5: Atomic Commit Hygiene' in text
print('Constitution Upstream Protocol Invariants: 100% VERIFIED!')
"
```

### Expected Output
```text
Constitution Upstream Protocol Invariants: 100% VERIFIED!
```

### Failure Diagnostic
- **Issue: Assertion Failed**:  lacks the complete Section 6 definition. Run the sync script to restore the canonical five upstream invariants.

---

## 3. Scenario 3: Educational Case Study & Knowledge Graph Navigation (US3)

### Command
```bash
python3 -c "
import os
case_study_path = 'docs/study/case_study_arm64_simd_journey.md'
assert os.path.exists(case_study_path), 'Case study document missing'
with open(case_study_path) as f:
    content = f.read()
assert '## 1. 案例背景与问题定义' in content
assert '## 2. 微架构物理机制剖析' in content
assert '## 3. 从 solo vmaxvq 到 Plan B 混合架构' in content
assert '## 4. 开源协作反思与致歉' in content
print('Educational Case Study Completeness: 100% VERIFIED!')
"
```

### Expected Output
```text
Educational Case Study Completeness: 100% VERIFIED!
```

### Failure Diagnostic
- **Issue: Missing Sections**: Ensure  is populated with the complete 4-stage narrative.
