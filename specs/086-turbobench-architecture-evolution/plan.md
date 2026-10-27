# Implementation Plan: TurboBench 4D Architecture Evolution Suite

**Feature Branch**: `086-turbobench-architecture-evolution`  
**Created**: 2026-08-18  
**Status**: Ready for Implementation  
**Specification**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/086-turbobench-architecture-evolution/spec.md)  
**Research**: [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/086-turbobench-architecture-evolution/research.md)  

---

## 1. Technical Context & Scope

### 1.1 Scope Boundaries
This feature incorporates TurboBench & lzbench architectural capabilities across 4 core dimensions:
1. **Pareto Frontier Analytics & Multi-Tier Visualization**:
   - 2D Skyline sweep-line algorithm and Monotone Convex Hull upper envelope calculation.
   - Zero-dependency standalone responsive SVG generator (`SVGParetoPlotter`).
   - $8\times$ sub-pixel Unicode Braille terminal chart renderer (`TerminalParetoPlotter`).
2. **Thermal Throttling & DVFS Guard**:
   - Asynchronous platform thermal coordinator (`HardwareThermalCoordinator`).
   - Adaptive exponential cool-down scheduling during sustained stress runs.
3. **Physical Media Turnaround Modeling (Transfer Speed Sheet)**:
   - Analytical calculation of end-to-end turnaround latency across Cloud WAN ($25\text{ MB/s}$), 1Gbps LAN ($125\text{ MB/s}$), 10Gbps LAN ($1250\text{ MB/s}$), and NVMe SSD ($3000\text{ MB/s}$).
   - Automated identification of the Pareto winner per medium.
4. **Smart Codec Scenario Selector & Virtual Memory Arena**:
   - 3-Stage cascaded entropy and compressibility prober ($< 0.5\text{ ms}$ on 1MB payload).
   - Scenario routing ("Instant Share", "Balanced Daily", "Cold Storage").
   - Contiguous Virtual Multi-Block Arena for small files ($< 64\text{ KB}$).

### 1.2 Constitution Check
- **Zero-Cost on Hot Paths**: Pareto calculation and SVG generation occur exclusively on benchmark completion/analysis phases; probers in hot paths use ARM NEON SIMD and zero heap allocations.
- **Thread Safety**: Thermal monitoring runs on detached Swift actors (`HardwareThermalCoordinator`), broadcasting lock-free atomic states to C runtime.
- **Frozen File Compliance**: No modifications to frozen ZIP engine files (`ZipParallelExtractor.swift`, `CTTZipExtract.c`, etc.).

---

## 2. Phase 0: Research Items (Resolved)

- [x] **R001 [SUBAGENT:research]**: 2D Pareto Frontier Skyline Algorithm & Monotone Convex Hull Optimization. *(Resolved in research.md: Kung-Luccio-Preparata 1975 + Andrew's Monotone Chain + Dilworth Multi-Tier Ranking)*.
- [x] **R002 [SUBAGENT:research]**: Zero-Dependency Standalone SVG Chart Generation & High-Resolution Unicode Braille/ASCII Terminal Scatter Plotting. *(Resolved in research.md: Log10 X-Axis + Linear Y-Axis + Responsive CSS + Braille $2\times 4$ Dot Canvas)*.
- [x] **R003 [SUBAGENT:research]**: macOS Apple Silicon Hardware Thermal State & DVFS Adaptive Cool-Down. *(Resolved in research.md: `AsyncSequence` notification listener + state-driven tail settling delay)*.
- [x] **R004 [SUBAGENT:research]**: Physical Media Turnaround Modeling & Sub-10ms Smart Codec Entropy Prober. *(Resolved in research.md: Turnaround Speedup Equation + 3-stage cascaded NEON & strided trial prober)*.

---

## 3. Phase 1: Design Artifacts & Contracts

- **Data Models**: Defined in [data-model.md](file:///Users/kevintung/Documents/dev/TTZip/specs/086-turbobench-architecture-evolution/data-model.md).
- **Contracts**:
  - `contracts/pareto-analysis.schema.json` [SUBAGENT:research]
  - `contracts/transfer-speed-sheet.schema.json` [SUBAGENT:research]
  - `contracts/smart-codec-recommendation.schema.json` [SUBAGENT:research]
- **Validation Guide**: Defined in [quickstart.md](file:///Users/kevintung/Documents/dev/TTZip/specs/086-turbobench-architecture-evolution/quickstart.md).

---

## 4. Components & File Modification Blueprint

### Component 1: `TTZipCore/Analytics/` (Pareto Frontier & Visualization Models)
- `Sources/TTZipCore/Benchmark/ParetoFrontierCalculator.swift`: [NEW] 2D Skyline and Monotone Convex Hull algorithm.
- `Sources/TTZipCore/Benchmark/SVGParetoPlotter.swift`: [NEW] Zero-dependency responsive dark/light SVG generator.
- `Sources/TTZipCore/Benchmark/TerminalParetoPlotter.swift`: [NEW] Unicode Braille sub-pixel terminal scatter plot renderer.
- `Sources/TTZipCore/Benchmark/TransferSpeedSheetCalculator.swift`: [NEW] Media turnaround latency projector.

### Component 2: `TTZipCore/Platform/` (Thermal Architecture)
- `Sources/TTZipCore/Platform/HardwareThermalCoordinator.swift`: [NEW] Swift 6 async actor for OS thermal monitoring and cooldown delays.
- `Sources/CTTZipBridge/include/CTTZipPlatform.h`: [MODIFY] Declare atomic thermal state accessors.
- `Sources/CTTZipBridge/CTTZipPlatform.c`: [MODIFY] Implement lock-free thermal state store/load.

### Component 3: `TTZipCore/Services/` (Smart Codec & Memory Arena)
- `Sources/TTZipCore/Services/SmartCodecSelector.swift`: [NEW] 3-stage cascaded entropy prober and scenario recommender.
- `Sources/TTZipCore/Buffer/VirtualMultiBlockArena.swift`: [NEW] Contiguous small-file batch arena allocator.

### Component 4: `TTZipCLI/` (Command Routing & Presentation)
- `Sources/TTZipCLI/CLIOptions.swift`: [MODIFY] Add `--pareto`, `--plot`, `--svg-out`, `--thermal-guard`, `--transfer-sheet`, `--recommend`, `--scenario`.
- `Sources/TTZipCLI/CLIArgumentParser.swift`: [MODIFY] Parse newly introduced CLI flags.
- `Sources/TTZipCLI/CLIBenchmarkRunner.swift`: [MODIFY] Dispatch Pareto plotting, SVG export, and transfer sheet formatting.

### Component 5: `Tests/TTZipTests/` (Unit & Regression Tests)
- `Tests/TTZipTests/ParetoFrontierCalculatorTests.swift`: [NEW] Unit tests for mathematical Pareto dominance and convex hull.
- `Tests/TTZipTests/SVGParetoPlotterTests.swift`: [NEW] W3C validation, dark/light CSS, and size constraints (< 25KB).
- `Tests/TTZipTests/TerminalParetoPlotterTests.swift`: [NEW] Braille dot rendering and ANSI layout validation.
- `Tests/TTZipTests/ThermalThrottlingGuardTests.swift`: [NEW] Thermal state transitions and cooldown simulation tests.
- `Tests/TTZipTests/SmartCodecSelectorTests.swift`: [NEW] Sub-10ms entropy probing and scenario mapping tests.
