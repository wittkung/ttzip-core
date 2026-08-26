# Quickstart & Verification Guide: TurboBench 4D Architecture Evolution Suite

**Feature**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/086-turbobench-architecture-evolution/spec.md)  
**Status**: Ready for Validation  

---

## 1. Scenario 1: Pareto Frontier & Terminal Unicode Plotting

### Command
```bash
swift run ttzip-cli bench --in-memory --compat-turbobench --pareto --plot -f zip,7z,zstd,lz4 -l 1,6
```

### Expected Output
- **Algorithm Execution**: In-memory multi-iteration passes completed on RAM buffers.
- **Terminal 2D Scatter Plot**: A clean Unicode Braille / box-framed scatter plot is printed with:
  - Y-axis: Space Savings % ($0\% \dots 100\%$)
  - X-axis: Throughput MB/s (Logarithmic scale $10 \dots 100,000\text{ MB/s}$)
  - 👑 Markers identifying Rank 1 Pareto points (e.g., LZ4 L1, ZSTD L1, ZSTD L6, 7Z-LZMA2 L6).
- **Tabular Legend**: Listing Pareto-optimal codecs with exact ranks.

### Failure Diagnostic
- If terminal plot wraps lines: Verify terminal column detection (`ttzip_get_terminal_width() >= 80`).
- If frontier missing: Verify `ParetoFrontierCalculator` output has non-empty `frontierPoints`.

---

## 2. Scenario 2: Zero-Dependency Standalone SVG Vector Graphic Export

### Command
```bash
swift run ttzip-cli bench --in-memory --compat-turbobench --svg-out docs/benchmarks/pareto_frontier.svg -f zip,7z,zstd,lz4 -l 1,3,6
```

### Expected Output
- **File Generation**: Valid SVG file written to `docs/benchmarks/pareto_frontier.svg`.
- **File Size**: Footprint is $< 25\text{ KB}$ (typically $8\text{ KB} \sim 15\text{ KB}$).
- **Browser Rendering**:
  - Open via `open docs/benchmarks/pareto_frontier.svg` in Safari / Chrome.
  - Hovering over any data point displays pure CSS vector tooltip showing Codec Name, Throughput, and Space Savings.
  - Automatically switches between dark and light themes when changing macOS Appearance.

### Failure Diagnostic
- If SVG is blank: Check XML tags balance (`<svg>...</svg>`) and CSS `<style>` syntax.
- If file $> 25\text{ KB}$: Verify coordinate formatting uses 1 decimal precision (`String(format: "%.1f", val)`).

---

## 3. Scenario 3: Physical Media Turnaround Projection (Transfer Speed Sheet)

### Command
```bash
swift run ttzip-cli bench --in-memory --transfer-sheet -f zip,zstd,lz4,7z -l 1,6
```

### Expected Output
- **Transfer Sheet Output**: Detailed matrix displaying total turnaround time across:
  - 10Gbps LAN ($1250\text{ MB/s}$)
  - 1Gbps LAN / 5G ($125\text{ MB/s}$)
  - NVMe SSD ($3000\text{ MB/s}$)
  - Cloud WAN ($25\text{ MB/s}$)
- **Winner Badges**: 👑 Marked beside the fastest turnaround codec per tier.

### Failure Diagnostic
- If turnaround time $> 100\text{s}$ on 10MB: Check speedup denominator math ($V_{\text{comp}}$ and $V_{\text{media}}$ units alignment).

---

## 4. Scenario 4: Smart Codec Scenario Selector & Rapid Entropy Probing

### Command
```bash
swift run ttzip-cli bench --recommend --scenario airdrop -i Sources/TTZipCore/Zip/ZipParallelExtractor.swift
```

### Expected Output
- **Execution Time**: Completes in $< 15\text{ ms}$.
- **Recommendation Card**:
  - Detected Shannon entropy (e.g. $4.85\text{ bits/byte}$) and 64KB trial ratio.
  - Recommended Codec: ZSTD L1 or Parallel Deflate L1 for AirDrop fast transfer.
  - Clear rationale and expected compression time.

### Failure Diagnostic
- If probe takes $> 50\text{ ms}$: Verify 64KB strided sampling is used instead of compressing full multi-megabyte payloads.
