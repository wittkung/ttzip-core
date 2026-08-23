# Tasks: Feature 131 - Upstream Contribution & Benchmark Verification Protocol

## Phase 1: Core Tooling & Verification Infrastructure

- [x] T001 [P] [US1] Create automated compiler flag parity validator in `scripts/upstream_crossover_bench.py`
- [x] T002 [P] [US1] Implement dual cross-over mirrored benchmark runner (A/B and B/A execution) in `scripts/upstream_crossover_bench.py`
- [x] T003 [P] [US2] Implement zero-hallucination dynamic markdown report generator in `scripts/upstream_report_gen.py`
- [x] T004 [US3] Formalize upstream contribution sovereignty and permission gate in `.agents/rules/upstream-contribution.md`
- [x] T005 [US4] Implement commit architecture, 7-char short SHA validator, and CTest/GTest pre-flight test in `scripts/upstream_audit_gate.py`

## Phase 2: Verification & End-to-End Testing

- [x] T006 [US1] Validate cross-over runner with synthetic identical binaries to assert 0.00% ± 0.2% parity convergence
- [x] T007 [US2] Validate report generator against edge-case JSON datasets to assert 100% parameterization
- [x] T008 [US3] Verify pre-flight audit gate and permission blocking in agentic workflows
