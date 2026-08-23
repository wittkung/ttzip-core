# Feature Specification: Pure C11 Core Engine (`libttzip`) & Cross-Platform (macOS + Windows) Architecture

**Feature Branch**: `143-pure-c-core-cross-platform-architecture`  
**Created**: 2026-08-20  
**Status**: Draft  
**Input**: User directive: "详细的整理到docs里，然后基于这个良好的设计开端开始设计完整的下沉方案和跨平台方案 并彻底实施 不要害怕工作量，一切以最高性能 最好体验为核心；而且我们要充分利用好mac平台，基于m系列芯片完整做好加速。同时也要利用好intel平台与nvidia显卡/amd显卡。"

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Pure C11 Cross-Platform Core Engine (`libttzip`) (Priority: P1) 🎯 MVP

As a developer on Windows, macOS, or Linux, I want a single, zero-external-dependency, pure C11 archiving library (`libttzip.a` / `ttzip.dll`) that provides full-featured, hardware-accelerated compression, extraction, and container manipulation for all 16 supported formats (ZIP, 7Z, TAR, GZ, XZ, ZST, BZ2, LZIP, LZ4, Brotli, Snappy, WIM, DMG, ISO), so that applications in any programming language (C, C++, C#, Swift, Rust, Python, Go) can achieve industry-leading archiving throughput without Swift runtime overhead or Apple-specific framework locks.

**Why this priority**:
Constitutes the physical foundation for world-class cross-platform expansion. Eliminates the platform lock caused by Swift/GCD while consolidating all single-core SOTA micro-kernels (`libdeflate`, `fast-lzma2`, `libzstd`, `liblz4`, `libbrotli`, `c-blosc2`).

**Independent Test**:
Can be tested independently by compiling `libttzip` via CMake under MSVC (Windows x64) and Clang (macOS ARM64/x86_64), running standalone C test suites across all container formats, and verifying identical bitstream output.

**Acceptance Scenarios**:
1. **Given** a pure C11 build environment without Swift or Apple frameworks, **When** `libttzip` is compiled via CMake, **Then** the build completes with zero errors, producing a self-contained static/dynamic library.
2. **Given** a multi-threaded archive creation request via `ttzip_archive_create()`, **When** executed on Windows or macOS, **Then** the engine utilizes the cross-platform thread pool (`ttzip_threadpool.c`) without calling GCD or POSIX-only spawn APIs.

---

### User Story 2 - Dual-ISA SIMD Acceleration & Hardware Vector Parity (Priority: P2)

As a power user running TTZip on x86_64 (Intel Mac or Windows PC) or ARM64 (Apple Silicon or Windows on ARM), I want all checksum and cryptographic hot paths (CRC64, CRC32, Adler-32, AES-256) to execute through native hardware vector instructions with automatic runtime CPU feature detection, so that x86_64 systems achieve parity with Apple Silicon (>40 GB/s CRC throughput) without manual configuration.

**Why this priority**:
Prevents x86_64 systems from degrading to slow scalar fallback implementations, guaranteeing world-class throughput across both major desktop CPU architectures.

**Independent Test**:
Can be tested by executing hardware checksum benchmark suites on both ARM64 and x86_64 machines and verifying that CRC64 achieves $\ge 40\text{ GB/s}$ via PMULL and PCLMULQDQ respectively.

**Acceptance Scenarios**:
1. **Given** an x86_64 CPU supporting PCLMULQDQ, **When** CRC64 is computed, **Then** the engine dynamically routes to `ttzip_crc64_x86_pclmul()` achieving $\ge 40.0\text{ GB/s}$.
2. **Given** an x86_64 CPU supporting AVX2, **When** Adler-32 is computed, **Then** the engine dynamically routes to `ttzip_adler32_x86_avx2()` achieving $\ge 30.0\text{ GB/s}$.
3. **Given** an ARM64 CPU, **When** CRC64/CRC32 are computed, **Then** the engine dynamically routes to PMULL and ACLE instructions achieving $\ge 48\text{ GB/s}$ and $\ge 65\text{ GB/s}$ respectively.

---

### User Story 3 - Full Engine Sinking & Thin Native GUI Integration (Priority: P3)

As an end user on macOS or Windows, I want a modern, high-performance native desktop application with instant startup (<0.001s cold start) and responsive UI, where the GUI layer is purely a thin presentation shell calling the C11 `libttzip` engine, so that the experience is seamless and native to each OS while sharing 100% of the underlying compression and verification logic.

**Why this priority**:
Ensures maximum maintainability and zero code divergence between macOS (SwiftUI/AppKit) and Windows (WinUI 3/C#) frontends.

**Independent Test**:
Can be tested by driving archive operations through both the macOS SwiftUI app and the standalone C CLI (`ttzip-cli`), verifying identical progress reporting, memory bounds, and archive output.

**Acceptance Scenarios**:
1. **Given** a 50GB file compression task initiated from the macOS GUI, **When** processed, **Then** `TTZipCore` delegates execution to `ttzip_archive_create()`, with progress callbacks updating the UI at 60 FPS while resident memory stays $\le 128\text{MB}$.
2. **Given** the standalone `ttzip-cli` executed on Windows or macOS, **When** invoked, **Then** it operates with zero Swift runtime dependency, starting instantly and completing archive operations at wire speed.

---

### User Story 4 - Heterogeneous GPU Acceleration for Massive Datasets (Priority: P4)

As a power user processing massive payloads ($\ge 64\text{MB} \sim \text{multi-GB}$ disk images, video packages, or AI tensor checkpoints), I want the engine to dynamically leverage GPU compute accelerators (NVIDIA CUDA / nvCOMP, Microsoft DirectStorage GDeflate for AMD/Intel, and Apple Metal 3 Compute Shaders with Unified Memory Zero-Copy), so that multi-gigabyte compression, decompression, and BLAKE3 tree verification scale into the tens and hundreds of gigabytes per second ($30 \sim 100+\text{ GB/s}$).

**Why this priority**:
Breaks through CPU execution port and cache line limits on massive datasets, achieving absolute peak throughput on modern GPU-equipped workstations and Apple Silicon Macs.

**Independent Test**:
Can be tested by running 10GB+ dataset compression benchmarks with GPU acceleration enabled and comparing throughput against CPU-only baselines.

**Acceptance Scenarios**:
1. **Given** an NVIDIA GPU on Windows/Linux with a payload $\ge 64\text{MB}$, **When** GDeflate / LZ4 compression is requested, **Then** the engine dynamically routes to `nvCOMP` achieving $\ge 50\text{ GB/s}$ throughput.
2. **Given** an Apple Silicon Mac with unified memory and a payload $\ge 64\text{MB}$, **When** compressed via Metal Compute Shaders, **Then** the engine executes with zero PCIe copy overhead directly on `MTLResourceStorageModeShared` memory buffers.
3. **Given** a payload $< 16\text{MB}$, **When** processed, **Then** the dynamic scheduler automatically routes to CPU SIMD to avoid GPU kernel launch overhead.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST implement `libttzip` as a standalone, zero-external-dependency Pure C11 core library with a versioned public C ABI defined in `include/ttzip_api.h`.
- **FR-002**: The engine MUST eliminate all C-layer calls to Apple GCD (`dispatch_queue_create`, `dispatch_async`, `dispatch_apply`, `dispatch_semaphore`, Blocks `^{}`), replacing them with the cross-platform `ttzip_threadpool.c` abstraction (supporting POSIX `pthread` and Win32 ThreadPool).
- **FR-003**: The engine MUST implement Dual-ISA hardware vector acceleration with runtime CPU feature detection (`ttzip_cpu_detect.c`), supporting ARM64 (NEON, PMULL, ACLE, AES) and x86_64 (SSE4.2, AVX2, AVX-512, PCLMULQDQ, AES-NI) with scalar fallbacks.
- **FR-004**: The engine MUST implement a cross-platform file system abstraction `ttzip_fs.h` supporting POSIX (`opendir`, `lstat`, `mmap`) and Win32 (`FindFirstFileW`, `CreateFileMappingW`, long path `\\?\` prefix up to 32,768 characters).
- **FR-005**: All container framing and demuxing logic (ZIP/Zip64, 7Z Solid/Coders DAG, TAR PAX, GZIP, XZ, ZSTD, WIM, DMG) MUST be implemented in C, residing within `libttzip`.
- **FR-006**: The engine MUST maintain 100% license compliance by using only permissive licenses (MIT, BSD, Apache-2.0, Public Domain), strictly excluding copyleft GPL-3 libraries (such as `lbzip2`).
- **FR-007**: The CMake build system (`CMakeLists.txt`) MUST support native generation of `libttzip.a` (static), `ttzip.dll` / `libttzip.so` (shared), and `ttzip-cli` (standalone executable) across MSVC, Apple Clang, and GCC.
- **FR-008**: The engine MUST implement an abstract heterogeneous compute dispatcher (`ttzip_compute_engine.h`) supporting dynamic CPU SIMD routing for payloads $< 16\text{MB}$ and GPU compute routing (NVIDIA nvCOMP, DirectStorage GDeflate, Apple Metal 3) for payloads $\ge 64\text{MB}$.
- **FR-009**: Swift `TTZipCore` on macOS MUST be refactored into a thin binding layer over `libttzip.a`, delegating all compression, extraction, and format parsing to the C engine.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `libttzip` builds successfully on Windows (MSVC x64/ARM64) and macOS (Clang ARM64/x86_64) via CMake with zero warnings under `/W4` / `-Wall -Wextra`.
- **SC-002**: 100% of C files have zero references to `<dispatch/dispatch.h>`, Apple Blocks syntax `^{}`, or Darwin-only headers without platform guards.
- **SC-003**: CRC64 throughput achieves $\ge 40\text{ GB/s}$ on x86_64 (PCLMULQDQ) and $\ge 48\text{ GB/s}$ on ARM64 (PMULL).
- **SC-004**: Multi-threaded compression throughput scaling achieves $\ge 85\%$ linear parallel efficiency across 8 to 32 physical cores on both Windows and macOS.
- **SC-005**: GPU-accelerated GDeflate/LZ4 decompression throughput on payloads $\ge 64\text{MB}$ reaches $\ge 35\text{ GB/s}$ (Apple Metal UMA) and $\ge 60\text{ GB/s}$ (NVIDIA RTX / AMD DirectStorage).
- **SC-006**: 100% of generated archives across all formats pass standard validation by external system oracles (`/usr/bin/unzip`, `/usr/bin/tar`, `7zz t`).
- **SC-007**: Resident memory consumption during 50GB streaming tasks remains strictly $\le 128\text{MB}$.
