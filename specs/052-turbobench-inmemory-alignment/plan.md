# Implementation Plan: TurboBench & lzbench In-Memory Benchmarking & High-Precision Timer Calibration Suite

**Branch**: `052-turbobench-inmemory-alignment` | **Date**: 2026-08-17 | **Spec**: [`spec.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/052-turbobench-inmemory-alignment/spec.md)

**Input**: Feature specification from `/specs/052-turbobench-inmemory-alignment/spec.md`

## Summary

Implement a pure in-memory benchmarking suite (`InMemoryBenchmarkEngine`), hardware-level cross-platform monotonic timer (`PlatformMonotonicTimer`), and standardized TurboBench / lzbench throughput and ratio calculation models within TTZip. This completely eliminates disk I/O, VFS lock contention, OS page-cache writeback jitter, and antivirus real-time scanning overheads, delivering industrial-grade micro-benchmarks with sub-microsecond precision and $<2.5\%$ measurement variance across macOS (Apple Silicon & Intel) and Windows platforms.

## Technical Context

**Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`) + C11 / POSIX APIs  
**Primary Dependencies**: `CTTZipBridge` (libdeflate, zstd, lzma, libarchive), zero external CLI subprocess dependencies  
**Storage**: Pure RAM (16KB page-aligned contiguous buffers allocated via `NativeCoreArchitecture.allocateAlignedPageBuffer`), zero physical disk persistence during benchmark loop  
**Testing**: `swift test --filter InMemoryBenchmarkSuiteTests`, `swift test --filter PlatformMonotonicTimerTests`  
**Target Platform**: macOS 14.0+ (Apple Silicon NEON prioritized, Intel x86_64 compatible), Windows 10/11 x64 & ARM64 (MSVC)  
**Project Type**: CLI & Core In-Process Benchmark Framework  
**Performance Goals**:
- In-memory benchmarking loop jitter $CV \le 2.5\%$ across 10 trials
- Timer resolution $< 100\text{ ns}$ with $< 0.1\%$ measurement overhead on $\ge 1\text{ ms}$ runs
- Roundtrip memory verification speed $\ge 30\text{ GB/s}$ via 64-byte NEON `memcmp` / hardware CRC32  
**Constraints**:
- Zero `malloc`/`free`, zero `Data(count:)`, zero ARC retain/release in the inner timing loop
- Zero `CACurrentMediaTime()` (QuartzCore) dependencies in CLI & core engine
- Zero bare objects in contracts, strict 128-bit arithmetic to prevent 64-bit timer tick overflow  
**Scale/Scope**: Matrix testing across all 16 compression algorithms, 500ms adaptive time-clamping, TurboBench/lzbench JSON report parity  

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Invariant / Check | Status | Verification Detail |
| :--- | :--- | :--- |
| **Zero-Cost Abstraction on Hot Paths** | PASS | Inner timing loop operates strictly on raw C pointers with persistent codec handles; zero heap allocation. |
| **Prohibited Anti-Patterns** | PASS | No `NSLock`/mutex inside timing loops, no `Data(count:)` zero-fill faults, no dynamic object trees. |
| **Fast-Path Bypass** | PASS | In-memory engine bypasses VFS/file systems directly into C codec compression/decompression functions. |
| **Throughput Floors** | PASS | Benchmark suite enforces TTZip hard floors (ZIP L1 $\ge 1500\text{ MB/s}$, LZ4 $\ge 6000\text{ MB/s}$, etc.). |
| **The Four Systemic Invariants** | PASS | 1. Stream-First (RAM-bounded working sets); 2. Invariant-First (deterministic buffers); 3. Bounds-First (128-bit overflow prevention, clamp); 4. Oracle-First (bit-exact roundtrip `memcmp`). |

## Project Structure

### Documentation (this feature)

```text
specs/052-turbobench-inmemory-alignment/
├── spec.md              # Feature specification
├── plan.md              # This file
├── research.md          # Phase 0 research findings (R001, R002, R003)
├── data-model.md        # Phase 1 data entities and telemetry models
├── quickstart.md        # Phase 1 validation guide
├── contracts/           # Phase 1 JSON Schema contracts
│   ├── inmemory-benchmark-request.schema.json
│   ├── inmemory-benchmark-result.schema.json
│   └── platform-timer-calibration.schema.json
├── checklists/
│   └── requirements.md  # Specification quality checklist
└── tasks.md             # Phase 2 implementation task list
```

### Source Code Changes

```text
Sources/
├── CTTZipBridge/
│   ├── include/CTTZipBridge.h             # Export ttzip_monotonic_nanos & timer init APIs
│   ├── CTTZipPlatformTimer.h             # [NEW] Cross-platform hardware monotonic timer header
│   └── CTTZipPlatformTimer.c             # [NEW] mach_absolute_time & QPC implementation
├── TTZipCore/
│   ├── Platform/
│   │   └── PlatformMonotonicTimer.swift   # [NEW] Swift high-precision monotonic timer abstraction
│   └── Benchmark/
│       ├── InMemoryBenchmarkEngine.swift  # [NEW] Pure in-memory benchmark harness & warmup runner
│       ├── InMemoryBenchmarkModels.swift  # [NEW] TurboBench/lzbench aligned data models
│       └── CompetitorBenchmarkRunner.swift# [MODIFY] Replace CACurrentMediaTime with PlatformMonotonicTimer
└── TTZipCLI/
    ├── CLICommandRouter.swift             # [MODIFY] Add --in-memory & --compat-turbobench flags
    └── CLIBenchmarkRunner.swift           # [MODIFY] Connect in-memory benchmark dispatch

Tests/TTZipTests/
├── InMemoryBenchmarkSuiteTests.swift      # [NEW] Unit & repeatability regression suite
└── PlatformMonotonicTimerTests.swift      # [NEW] Hardware timer resolution & drift tests
```

## Phase 0: Outline & Research

- R001 [SUBAGENT:research] 《内存基准测试生命周期与预热机制》：研究 TurboBench / lzbench 预分配连续内存、预热轮次与 500ms 自适应时间夹紧算法。
- R002 [SUBAGENT:research] 《跨平台硬件纳秒级单调时钟校准》：研究 macOS `mach_absolute_time`、Windows `QueryPerformanceCounter` 与 POSIX `CLOCK_MONOTONIC_RAW` 纳秒转换与 64 位防溢出算法。
- R003 [SUBAGENT:research] 《吞吐量公式、统计收敛与报告格式对齐》：研究 TurboBench / lzbench 十进制 MB/s 公式、最小耗时峰值吞吐模型与内存逐字节校验。

## Phase 1: Design & Contracts

- Data Model: `specs/052-turbobench-inmemory-alignment/data-model.md`
- Contracts [SUBAGENT:research]:
  - `contracts/inmemory-benchmark-request.schema.json`
  - `contracts/inmemory-benchmark-result.schema.json`
  - `contracts/platform-timer-calibration.schema.json`
- Quickstart Guide: `specs/052-turbobench-inmemory-alignment/quickstart.md`
