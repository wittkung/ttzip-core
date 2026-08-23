# Feature Specification: TurboBench 4D Architecture Evolution Suite

**Feature Branch**: `086-turbobench-architecture-evolution`  
**Created**: 2026-08-18  
**Status**: Draft  
**Input**: Comprehensive adoption of TurboBench & lzbench architectural insights across 4 core dimensions: (1) Benchmarking Rigor (Thermal Guard, Pareto Frontier Envelope Analysis, Multi-Tier Visual Charts including Unicode Terminal Plot, Standalone SVG and SwiftUI Charts, and Transfer Speed Sheets for 10G/5G/SSD/HDD), (2) Micro-Architecture & Memory (Virtual Multi-Block Arena for Small Files and Dictionary Attaching), (3) Next-Gen Fast-Path Codecs (Fast-LZMA2 multi-core acceleration and bzip3 Radix-BWT integration), and (4) Product & Systemic Empowerment (Smart Codec Scenario Selector with lightweight entropy probing).

## Clarifications

### Session 2026-08-18

- Q: How should the Pareto Frontier be defined and computed mathematically?
  → A: A candidate (Algorithm, Level) is Pareto-optimal if no other candidate has both strictly greater or equal compression space savings (%) AND strictly greater or equal throughput (MB/s), with at least one metric strictly greater. The Pareto frontier sequence is computed via 2D Monotone Convex Hull / Skyline filter over benchmark data points.
- Q: What visual chart formats should be generated for the Pareto analysis?
  → A: Triple-tier rendering: (1) High-resolution Unicode Braille/ASCII scatter plot directly in CLI stdout (`--plot`), (2) Zero-dependency interactive SVG vector graphic file (`--svg-out`), and (3) Decoupled Codable chart data models prepared for desktop SwiftUI `Swift Charts`.
- Q: How should CPU thermal throttling and DVFS scaling jitter be mitigated during continuous stress benchmarking?
  → A: On macOS, monitor `ProcessInfo.processInfo.thermalState`. When thermal pressure enters `.serious` or `.critical`, the engine automatically pauses iteration loops and enters adaptive cooling sleep until thermal state returns to `.nominal` / `.fair`, ensuring all algorithms execute under peak CPU Turbo frequencies.
- Q: How does the Transfer Speed Sheet project real-world latency?
  → A: Calculate total turnaround time $T_{\text{total}} = \frac{\text{Size}_{\text{orig}}}{V_{\text{comp}}} + \frac{\text{Size}_{\text{comp}}}{V_{\text{bandwidth}}} + \frac{\text{Size}_{\text{orig}}}{V_{\text{decomp}}}$ across standard physical tiers (10Gbps LAN, 1Gbps LAN / 5G, NVMe SSD, HDD / WAN), ranking algorithms by lowest overall turnaround time per medium.
- Q: How should Smart Codec Scenario Selector make automated recommendations?
  → A: Execute a non-destructive 1MB RAM-resident entropy and compressibility probe in $< 10\text{ ms}$, then match against the user's operational priority (Instant Sharing / Daily Balance / Cold Archive) based on calibrated Pareto points.

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Pareto Frontier Analysis & Triple-Tier Visualization (Priority: P1)

As a performance engineer and technical decision maker, I want `ttzip-cli bench` to automatically calculate the Pareto-optimal envelope across all tested algorithms and generate high-fidelity visual charts (terminal Unicode plot and standalone interactive SVG), so that I can visually identify optimal algorithms (such as AI quality vs. cost frontier curves) and eliminate dominated sub-optimal codecs.

**Why this priority**: Raw numerical benchmark tables require high cognitive overhead to compare. Visual Pareto envelopes instantly highlight non-dominated winners across space savings and speed trade-offs.

**Independent Test**: Run `swift run ttzip-cli bench --in-memory --pareto --plot --svg-out pareto.svg` and verify the terminal displays a clear 2D Unicode scatter plot with 👑 Pareto vertex markers, and generates a valid, self-contained SVG file with interactive tooltips.

**Acceptance Scenarios**:
1. **Given** benchmark results for 10+ algorithm/level combinations, **When** computing the Pareto frontier, **Then** the engine identifies the convex upper boundary points and flags dominated sub-optimal points.
2. **Given** the `--plot` flag, **When** outputting to terminal, **Then** a 2D ASCII/Unicode grid is rendered with logarithmic throughput on the X-axis, space savings on the Y-axis, and legend showing Pareto-optimal points.
3. **Given** `--svg-out <path>`, **When** exported, **Then** a clean, standalone SVG vector diagram (dark/light theme responsive) with axis labels, grid lines, plotted points, and the Pareto frontier line is generated without external JavaScript libraries.

---

### User Story 2 - Thermal Throttling Guard & Transfer Speed Sheet (Priority: P2)

As a benchmarking analyst running sustained performance test suites, I want the benchmarking engine to actively monitor hardware thermal states to prevent CPU throttling jitter, and output a physical media transfer speed projection sheet, so that benchmark measurements remain unpolluted by DVFS downclocking and reflect real-world end-to-end user turnaround times.

**Why this priority**: Extended benchmark suites heat up mobile and desktop chips, causing late-running algorithms to be unfairly penalized by CPU throttling. Transfer speed projections translate raw MB/s into real-world business value (e.g. AirDrop vs cloud upload times).

**Independent Test**: Run `swift run ttzip-cli bench --in-memory --thermal-guard --transfer-sheet` and verify thermal monitoring logs during heavy runs, followed by a structured matrix detailing total turnaround latency across 10G LAN, 1G LAN / 5G, NVMe SSD, and HDD.

**Acceptance Scenarios**:
1. **Given** heavy multi-core benchmark loops, **When** system thermal state elevates to `.serious` or `.critical`, **Then** the engine logs a warning, inserts adaptive cooling intervals, and resumes once normal thermal state is restored.
2. **Given** compressed size, compression speed, and decompression speed, **When** generating the transfer speed sheet, **Then** turnaround times for 10Gbps LAN ($1250\text{ MB/s}$), 1Gbps LAN ($125\text{ MB/s}$), NVMe SSD ($3000\text{ MB/s}$), and Cloud WAN ($25\text{ MB/s}$) are computed with Pareto winners highlighted.

---

### User Story 3 - Smart Codec Scenario Selector & Virtual Multi-Block Arena (Priority: P3)

As an end-user compressing large folders or diverse assets, I want TTZip to automatically analyze data entropy and recommend the Pareto-optimal codec based on my intended use case (Instant AirDrop Sharing, Daily Archive, Cold Backup), leveraging a virtual multi-block memory arena for small files, so that I get maximum speed and compression without manual parameter tuning.

**Why this priority**: Non-technical users cannot determine whether to choose ZSTD L3, LZ4, or 7Z L9. Automatic scenario-based recommendation democratizes advanced compression intelligence.

**Independent Test**: Run `swift run ttzip-cli bench --recommend --scenario airdrop -i /path/to/files` and verify the tool performs quick entropy probing and returns the optimal codec recommendation in $< 50\text{ ms}$.

**Acceptance Scenarios**:
1. **Given** a directory containing thousands of small files ($< 64\text{ KB}$), **When** scanning and staging in memory, **Then** memory allocation uses a contiguous virtual multi-block arena rather than per-file allocations.
2. **Given** user selected scenario "Fast Sharing" (AirDrop/10G LAN), **When** smart selector runs, **Then** it recommends high-decompression-throughput codecs (ZSTD L1 / LZ4), while for "Cold Storage", it recommends maximal ratio codecs (7Z-LZMA2 / bzip3).

---

### Edge Cases

- **Collinear and Equivalent Pareto Points**: Handling multiple algorithms that achieve identical space savings and throughput without duplicate frontier lines or infinite slopes.
- **Extreme High-Speed Algorithms (> 50 GB/s on RAM)**: Ensuring SVG and terminal chart X-axis scaling smoothly accommodates dynamic ranges from 10 MB/s to 100,000 MB/s (using base-10 logarithmic scaling).
- **Zero/Negative Space Savings**: Uncompressible data (already compressed videos/encrypted blobs) correctly plotted at $\le 0\%$ space savings without crashing chart layout.
- **Thermal Sensor Unavailable / Unsupported OS**: Seamless fallback to non-throttled timing when running in environments where hardware thermal APIs are inaccessible.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST implement a 2D Pareto frontier skyline algorithm (`ParetoFrontierCalculator`) that accepts benchmark results and computes the non-dominated subset maximizing space savings (%) and throughput (MB/s).
- **FR-002**: The system MUST implement a terminal-based Unicode/ASCII chart renderer (`TerminalParetoPlotter`) that renders a 2D scatter plot with logarithmic X-axis and Pareto frontier line directly to stdout.
- **FR-003**: The system MUST implement a zero-dependency SVG chart generator (`SVGParetoPlotter`) that produces valid, standalone, dark/light theme responsive SVG diagrams with hover tooltips and clear Pareto envelopes.
- **FR-004**: The system MUST implement a platform hardware thermal monitor (`ThermalThrottlingGuard`) that inspects OS thermal state (`ProcessInfo.thermalState` on macOS) and inserts adaptive cooling pauses during sustained benchmark execution when thermal pressure is detected.
- **FR-005**: The system MUST implement a transfer speed projection engine (`TransferSpeedSheetCalculator`) that computes end-to-end turnaround latency across standard physical media tiers (10Gbps LAN, 1Gbps / 5G, NVMe SSD, Cloud WAN) and identifies the Pareto winner per medium.
- **FR-006**: The system MUST implement a Smart Codec Selector (`SmartCodecSelector`) that evaluates sample data entropy and compressibility in $< 10\text{ ms}$ and maps to user scenarios: "Instant Transfer" (AirDrop/LAN), "Balanced Daily", and "Cold Backup".
- **FR-007**: The system MUST provide a virtual multi-block memory arena (`VirtualMultiBlockArena`) for contiguous batch staging of small files ($< 64\text{ KB}$) to eliminate heap fragmentation in mass-file operations.
- **FR-008**: The CLI MUST expose dedicated flags: `--pareto`, `--plot`, `--svg-out <path>`, `--thermal-guard`, `--transfer-sheet`, and `--recommend --scenario <name>`.
- **FR-009**: All Pareto data, transfer sheets, and scenario recommendations MUST be serializable to JSON according to strong JSON schemas in `contracts/`.

### Key Entities

- **ParetoPoint**: Represents a 2D data point (algorithm name, level, throughput MB/s, space savings %, isParetoOptimal, rank).
- **ParetoFrontierResult**: Encapsulates the complete dataset of evaluated points alongside the ordered Pareto optimal envelope subset.
- **ThermalGuardConfig**: Encapsulates thermal thresholds, poll frequency, and cooldown sleep duration.
- **TransferSpeedTier**: Defines media bandwidth specifications and calculated compression, transfer, decompression, and total latency.
- **ScenarioRecommendation**: Encapsulates chosen scenario, detected data entropy, recommended algorithm/level, and expected time/size savings.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Pareto frontier calculation over 50+ benchmark points executes in $< 1.0\text{ ms}$ with 100% mathematical correctness (verified against convex hull reference).
- **SC-002**: Terminal Unicode chart renders cleanly across 80-column to 200-column terminal windows with zero text wrapping corruption.
- **SC-003**: Generated SVG files are $< 25\text{ KB}$, 100% W3C SVG valid, render without external network assets, and open cleanly in all standard browsers.
- **SC-004**: Thermal throttling guard eliminates multi-pass throughput decay, maintaining measurement coefficient of variation $CV \le 2.0\%$ across 30-minute stress runs.
- **SC-005**: Smart Codec Selector completes entropy profiling and recommendation on 10MB data in $< 15\text{ ms}$.
