# Tasks: SPM Benchmark Target Isolation & `ttzip-bench` CLI Tooling

**Feature Directory**: `specs/136-spm-benchmark-target-isolation-and-cli-tooling`  
**Target Subject**: SPM 物理拆分、`TTZipBench` 独立可执行模块与核心库瘦身  
**Status**: Completed  

---

## Phase 1: Package Configuration & Directory Setup (Priority: P1)

- [x] T001 [P] [US1] Update `Package.swift` to add `ttzip-bench` executable product and `TTZipBench` executable target
- [x] T002 [P] [US1] Create directory `Sources/TTZipBench/` and subdirectories `Plotters/`, `Runners/`

---

## Phase 2: Code Migration & Decoupling from TTZipCore (Priority: P1)

- [x] T003 [P] [US2] Move `RasterParetoPlotter.swift`, `SVGParetoPlotter.swift`, `TerminalParetoPlotter.swift` to `Sources/TTZipBench/Plotters/`
- [x] T004 [P] [US2] Move and refactor `ExhaustiveBenchmarkRunner.swift` to `Sources/TTZipBench/Runners/` using `PlatformMonotonicTimer`
- [x] T005 [P] [US2] Remove migrated files from `Sources/TTZipCore/` and verify `TTZipCore` builds cleanly without GUI imports

---

## Phase 3: Implement `ttzip-bench` CLI Router (Priority: P1)

- [x] T006 [P] [US1] Implement `@main struct TTZipBenchApp` in `Sources/TTZipBench/main.swift` supporting `matrix`, `plot`, `gate`, `help` and `--json-out`
- [x] T007 [US1] Build and test `swift run ttzip-bench matrix` verifying 50-point matrix execution in $\le 1.2$ seconds

---

## Phase 4: Full Suite Verification & Convergence (Priority: P2)

- [x] T008 [US3] Run full test suite `swift test` and confirm 100% pass with 0 failures
- [x] T009 Verify all 16 checklist requirements in `checklists/requirements.md`
- [x] T010 Run `@speckit-converge` and `@speckit-analyze` consistency checks
