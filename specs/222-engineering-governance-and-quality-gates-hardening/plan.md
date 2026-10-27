# Implementation Plan: Engineering Governance and Quality Gates Hardening

**Branch**: `222-engineering-governance-and-quality-gates-hardening` | **Date**: 2026-08-24 | **Spec**: [`spec.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/222-engineering-governance-and-quality-gates-hardening/spec.md)

**Input**: Feature specification from `/specs/222-engineering-governance-and-quality-gates-hardening/spec.md`

---

## 1. Summary

Establish deterministic quality gates and stress testing suites to harden TTZip against cross-language symbol dropouts, regression in differential transactional rollbacks, memory residue in sensitive contexts, and split archive streaming bottlenecks.

---

## 2. Technical Context

- **Language/Version**: Swift 6.0, Rust 1.80+ (2021 Edition), C11 / C++17
- **Primary Dependencies**: `libarchive`, `libdeflate`, `zstd`, `lz4`, `lzfse`, `fast-lzma2`, `snap`, `brotli`
- **Storage**: APFS (macOS native clonefile CoW), virtual in-memory segmented stream buffers
- **Testing**: `swift test`, `cargo test`, `proptest`, `nm -gU` symbol gate, LLVM AddressSanitizer/ThreadSanitizer
- **Target Platform**: macOS 14.0+ (Apple Silicon arm64 & x86_64)
- **Project Type**: Native macOS High-Performance Archive Engine & Desktop App

---

## 3. Constitution & Gate Invariants

1. **LOC Defense Invariant**: Every single file must remain $\le 800$ LOC.
2. **C-ABI Export Parity Invariant**: 100% of symbols in `Sources/CTTZipBridge/include/ttzip_rust_glue.h` must exist in `Vendor/TTZipVendor.xcframework/macos-arm64/libTTZipVendor.a`.
3. **Differential Rollback Invariant**: Transactional rollback must remove only the newly created delta files with $O(\Delta)$ time/space, without prior full directory copy.
4. **Zero-Disk Split Stream Invariant**: Multi-volume `.001` archive inspection and extraction must never stage temporary concatenated files on disk.

---

## 4. Implementation Structure

```text
specs/222-engineering-governance-and-quality-gates-hardening/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
└── contracts/
    └── cabi-symbol-manifest.json

scripts/
├── verify_cabi_symbols.sh     # [NEW] Automated C-ABI symbol parity gate
├── run_sanitizers.sh          # [NEW] ASan / TSan test runner
└── run_local_ci_gate.sh       # [MODIFY] Insert verify_cabi_symbols stage

Tests/
└── TTZipTests/
    ├── LargeVolumeStressTests.swift      # [NEW] Synthetic 10k entry & 3-volume streaming tests
    └── CABISymbolGateTests.swift         # [NEW] Automated swift test for C-ABI symbol parity
```
