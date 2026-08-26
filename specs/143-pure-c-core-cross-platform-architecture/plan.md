# Implementation Plan: Pure C11 Core Engine (`libttzip`) & Cross-Platform Architecture

**Branch**: `143-pure-c-core-cross-platform-architecture` | **Date**: 2026-08-20 | **Spec**: [spec.md](spec.md)

**Input**: Sinking all archiving orchestration, container framing, and parallel scheduling to a pure C11 core library (`libttzip`), replacing Apple GCD with a self-hosted cross-platform thread pool, implementing Dual-ISA SIMD acceleration, and decoupling native UI shells.

---

## Summary

This plan outlines the systematic implementation of `libttzip` as a high-performance, cross-platform C11 library. It establishes portable thread pooling, implements Dual-ISA hardware vectorization (ARM NEON + x86_64 SSE4.2/AVX2/AVX-512/PCLMULQDQ), abstracts file system and memory-mapped I/O, sinks all container framing logic from Swift into C, and exposes a clean public C API (`ttzip_api.h`).

---

## Technical Context

**Language/Version**: ISO C11 (`-std=c11` / `/std:c11`) + ARM64 Assembly (`arm64inc.S`) + x86_64 Assembly / Intrinsics (`<immintrin.h>`).  
**Primary Dependencies**: Static C libraries (`libdeflate`, `fast-lzma2`, `libzstd`, `liblz4`, `libbrotli`, `c-blosc2`, `xxHash`, `libarchive`).  
**Storage & I/O**: POSIX `mmap` / Win32 `MapViewOfFile`, POSIX `lstat` / Win32 `FindFirstFileW`, UTF-8/UTF-16 path conversions.  
**Build Systems**: `CMakeLists.txt` (universal C engine build) + `Package.swift` (macOS Swift GUI shell wrapper) + `TTZip.sln` (Windows GUI shell wrapper).  
**Performance Goals**: $\ge 40\text{ GB/s}$ x86_64 CRC64, $\ge 48\text{ GB/s}$ ARM64 CRC64, $>4,000\text{ MB/s}$ Deflate multi-core, $<0.001\text{ ms}$ cold start.  
**Constraints**: Zero Swift/GCD dependencies in `libttzip`, zero heap allocation in hot loops, memory cap $\le 128\text{MB}$ per streaming task.

---

## Constitution Check

- [x] **Core Architecture Invariant**: 100% in-process static C bindings, zero external CLI subprocess execution.
- [x] **Zero-Cost Hot Path Invariant**: Zero dynamic heap allocations in inner compression loops; direct pointer passes and stack buffers only.
- [x] **Stream-First Invariant**: Micro-buffering pull pipeline ($\le 64\text{MB}\sim 128\text{MB}$ peak memory).
- [x] **Invariant-First Invariant**: POSIX and Win32 secure path traversal checks (`ARCHIVE_EXTRACT_SECURE_NODOTDOT`, `ARCHIVE_EXTRACT_SECURE_NOABSOLUTEPATHS`).
- [x] **Bounds-First Invariant**: Magic byte validation on C struct handles, zeroization of credentials via `ttzip_secure_zero`.
- [x] **Oracle-First Invariant**: Byte-accurate validation against external standard oracles (`/usr/bin/tar`, `/usr/bin/unzip`, `7zz`).
- [x] **Dual-ISA Parity**: Micro-architectural parity between ARM64 and x86_64 vector instruction sets.

---

## Project Structure & Source Layout

```text
Sources/CTTZipBridge/
├── include/
│   ├── ttzip_api.h                    # Versioned Public C API
│   ├── ttzip_platform.h               # Cross-platform macros, types, page alignment
│   ├── ttzip_windows.h                # Win32 UTF-8/16 & long path helpers
│   ├── ttzip_fs.h                     # Cross-platform File System & mmap abstraction
│   ├── ttzip_threadpool.h             # Cross-platform C11 thread pool API
│   ├── ttzip_cpu_detect.h             # Dual-ISA CPU feature detection
│   └── ttzip_codec.h                  # Single-core SOTA codec VTable
├── platform/
│   ├── ttzip_cpu_detect.c             # CPUID / getauxval / sysctl runtime feature detection
│   ├── ttzip_fs_posix.c               # POSIX opendir/lstat/mmap implementation
│   ├── ttzip_fs_win32.c               # Win32 FindFirstFileW/MapViewOfFile/\\?\ implementation
│   └── ttzip_threadpool.c             # pthreads + Win32 ThreadPool implementation
├── hardware/
│   ├── ttzip_crc64.c                  # Dual-ISA CRC64 dispatch (PMULL vs PCLMULQDQ vs Scalar)
│   ├── ttzip_crc64_arm64_pmull.c      # ARM64 PMULL 4-way vector fold (48.16 GB/s)
│   ├── ttzip_crc64_x86_pclmul.c       # x86_64 PCLMULQDQ 4-way vector fold (40+ GB/s)
│   ├── ttzip_crc32.c                  # Dual-ISA CRC32 dispatch (ACLE vs SSE4.2 vs libdeflate)
│   ├── ttzip_adler32.c                # Dual-ISA Adler-32 dispatch (NEON DotProd vs AVX2)
│   └── ttzip_aes.c                    # Dual-ISA AES-256 dispatch (ARM Crypto vs AES-NI)
├── parallel/
│   ├── ttzip_parallel_compress.c      # Universal chunked parallel compressor
│   ├── ttzip_dict_overlap.c           # Zero-copy sliding ring dictionary buffer
│   └── ttzip_bitstream_seq.c          # Format-aware bitstream sequencer (BFINAL management)
├── containers/
│   ├── ttzip_zip.c                    # Full ZIP/Zip64 container writer & parser
│   ├── ttzip_7z.c                     # Full 7Z solid container writer & parser
│   └── ttzip_tar.c                    # Full TAR PAX 512B container stream writer & parser
└── core/
    ├── ttzip_archive_create.c         # High-level archive creation pipeline
    ├── ttzip_archive_extract.c        # High-level archive extraction pipeline
    └── ttzip_archive_list.c           # High-level archive listing pipeline
```

---

## Phase 0: Research Items

- - R001 [SUBAGENT:research] 《Cross-Platform Thread Pool Architecture》: Design and benchmarking of lock-free work-stealing thread pools using POSIX `pthread` and Win32 `CreateThreadpoolWork`.
- - R002 [SUBAGENT:research] 《x86_64 SIMD Vectorization Implementation》: Exact PCLMULQDQ, SSE4.2 CRC32, and AVX2 Adler-32 vector implementations with Barrett reduction.
- - R003 [SUBAGENT:research] 《Win32 Long Path & Memory Mapped I/O Architecture》: `FindFirstFileW`, `CreateFileMappingW`, `PrefetchVirtualMemory`, and `\\?\` prefix integration.

---

## Phase 1: Design & Contracts

- `data-model.md`: Domain models for `TTZipArchiveConfig`, `TTZipThreadPoolDescriptor`, `TTZipFSEntry`, `TTZipHardwareDescriptor`.
- `contracts/`: Strongly-typed Draft-07 JSON Schemas:
  - `contracts/archive-config-schema.json`: Public C API archive creation options contract.
  - `contracts/threadpool-schema.json`: Thread pool configuration and thread pinning contract.
  - `contracts/fs-entry-schema.json`: Cross-platform file entry metadata contract.
  - `contracts/hardware-dispatch-schema.json`: CPU feature flags and SIMD vector dispatch contract.
- `quickstart.md`: Standalone CMake build instructions, C test suite commands, and verification assertions.
