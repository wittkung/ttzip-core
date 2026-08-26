# Feature Specification: 097-cross-block-deflate-dictionary-preconditioning

## Overview & Context

This feature formalizes, optimizes, and thoroughly validates the **Cross-Block Deflate Sliding Window Dictionary Preconditioning Engine** across TTZip.

When performing multi-core parallel Deflate block compression (e.g. in `ZipExtremeBlockWriter.swift`), independent block boundaries traditionally lose the 32KB LZ77 dictionary history of the preceding block, suffering a 1%–3% compression ratio penalty. By caching thread-local `z_stream` instances with `deflateReset` (avoiding expensive `deflateInit2`/`deflateEnd` allocations) and injecting the preceding block's trailing 32KB window via RFC 1951 `deflateSetDictionary`, TTZip achieves **parallel multi-gigabyte throughput with serial-grade compression ratios**.

---

## User Scenarios & Personas

### Scenario 1: Large File Compressor (High Throughput & Maximum Ratio)
- **Goal**: Compress a 100MB+ log file or dataset using all CPU cores in under 50ms without suffering compression ratio degradation.
- **Experience**: Throughput remains $\ge 2000\text{ MB/s}$ while output archive size is within 0.5% of serial single-threaded Deflate level 6/9.

### Scenario 2: Standard Unarchiver Interoperability (100% RFC 1951 / PKWARE Compliance)
- **Goal**: Ensure the resulting archive is 100% compliant with standard macOS Archive Utility, `/usr/bin/unzip`, `ditto`, `7zz`, and Linux `unzip`.
- **Experience**: All reference oracles extract the archive with 100% bit-exact SHA-256 integrity.

---

## Functional Requirements

- **FR-001**: Thread-local `s_tls_raw_deflate_strm` caching: reuse `z_stream` state per compression level across blocks via `deflateReset()`.
- **FR-002**: 32KB cross-block history injection: for chunk index $i > 0$, inject $\min(32768, \text{offset})$ trailing bytes from block $i-1$ via `deflateSetDictionary()`.
- **FR-003**: RFC 1951 `Z_SYNC_FLUSH` boundary emission for intermediate blocks and `Z_FINISH` for the terminal block.
- **FR-004**: Multi-way differential verification against `/usr/bin/unzip` and `ditto` for large multi-block files ($\ge 4\text{MB}$ across $\ge 4$ blocks).
- **FR-005**: Zero performance regression across all 13 constitutional performance gates.

---

## Success Criteria

| Metric | Target Baseline | Verification Method |
| :--- | :--- | :--- |
| **Cross-Block Deflate Ratio Gain** | $\ge 2.0\%$ improvement vs raw isolated blocks on repetitive corpus | `CrossBlockDeflateDictionaryTests` |
| **Multi-Core Throughput** | $\ge 2,000\text{ MB/s}$ (Debug) / $\ge 3,500\text{ MB/s}$ (Release) | `XCTestPerformanceMeasureTests` |
| **System Oracle Consensus** | 100% SHA-256 match vs `/usr/bin/unzip` and `ditto` | `DifferentialOracleTests` |
