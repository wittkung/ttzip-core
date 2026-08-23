# Quickstart & Verification: Feature 104 (ZIP Iterative Zopfli Conquest)

## Verification Scenario 1: Multi-Core Pareto Frontier Benchmark
- **Command**:
  ```bash
  swift test --filter ZipMultiCoreParetoFrontierPkTests
  ```
- **Expected Output**:
  - `📊 [TTZip Tier 0] Store (0): speed >= 6000 MB/s`
  - `📊 [TTZip Tier 1] Fast (1): speed >= 5000 MB/s`
  - `📊 [TTZip Tier 2] Fast+ (2): speed >= 5000 MB/s`
  - `📊 [TTZip Tier 3] Normal (3): speed >= 4000 MB/s`
  - `📊 [TTZip Tier 4] Maximum (4): speed >= 2000 MB/s`
  - `📊 [TTZip Tier 5] Graph Fast (5): speed >= 400 MB/s`
  - `📊 [TTZip Tier 6] Ultra Zopfli (6): speed >= 4.0 MB/s, sz <= 2.99 MB` (Strictly to upper-right of pigz-11)
  - `📊 [TTZip Tier 7] Extreme Peak (7): speed >= 1.5 MB/s, sz <= 2.95 MB` (Strictly to upper-right of advzip-4)
- **Failure Diagnostic**:
  - If output size exceeds 2.99 MB, verify whether `num_iterations` in `ZipCompressionProfile` is $\ge 10$.

## Verification Scenario 2: Integrity & System Unzip Standard Compliance
- **Command**:
  ```bash
  swift test --filter ZipExtremeBlockWriterTests
  ```
- **Expected Output**:
  - `[ZipExtremeBlockWriter] 10MB -> OK`
  - `[/usr/bin/unzip -t]: No errors detected`
- **Failure Diagnostic**:
  - If `/usr/bin/unzip -t` fails with CRC mismatch, check RFC 1951 Deflate stream termination and block concatenation.
