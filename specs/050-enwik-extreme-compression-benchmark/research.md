# Technical Research: enwik8 / enwik9 Extreme Compression Benchmark

**Feature**: `050-enwik-extreme-compression-benchmark`
**Created**: 2026-08-17
**Status**: Completed

---

## R001: Zero-Overhead Memory Telemetry (Peak RSS & Virtual Memory)

### Decision
Implement a zero-allocation, thread-safe memory telemetry snapshot provider in `PlatformMemory` (`Sources/TTZipCore/Platform/PlatformMemory.swift`):
- **macOS / Darwin**: Execute Darwin Mach kernel trap `task_info` with flavor `MACH_TASK_BASIC_INFO` against `mach_task_self_` using a stack-allocated `mach_task_basic_info_data_t`. Retrieve `resident_size` (current RSS in bytes), `resident_size_max` (peak lifetime RSS / high-water mark in bytes), and `virtual_size` (virtual memory in bytes).
- **Linux / Cross-Platform**: Parse `/proc/self/statm` using a fixed 64-byte stack buffer combined with `getrusage(RUSAGE_SELF, &usage)` (converting `ru_maxrss * 1024` to bytes). Standardize all outputs to UInt64 bytes within `MemoryCeilingSnapshot`.

### Rationale
1. **Zero Heap Allocation & Hot-Path Isolation**: `mach_task_basic_info` is a 48-byte C struct instantiated on the stack. The `task_info()` call is a direct kernel trap that causes zero heap allocations, zero dynamic dispatch, and zero ARC churn.
2. **True Lifetime High-Water Mark (Peak RSS)**: Unlike periodic timer polling which can miss microsecond allocation spikes, `resident_size_max` tracks the kernel-recorded lifetime maximum physical memory footprint of the process. This provides 100% deterministic validation for LZMA2 (Levels 5~9) and ZSTD Ultra dictionary memory limits.
3. **Cross-Platform Parity**: Standardizing units to bytes across Darwin and Linux ensures identical assertions across macOS Apple Silicon and Linux CI runners.

### Alternatives Considered
1. **Foundation `ProcessInfo.processInfo.physicalMemory`**:
   *Why rejected*: Only reports total physical RAM installed on the machine, not the process's resident or virtual memory consumption.
2. **Periodic Polling Thread / Timer (`DispatchSourceTimer`)**:
   *Why rejected*: Introduces runtime thread contention and scheduling jitter on benchmark threads. Microsecond peak allocations during dictionary setup are easily missed between sampling intervals.
3. **Subprocess Spawning (`/usr/bin/vm_stat` or `ps`)**:
   *Why rejected*: Spawning a child process introduces 10~50ms latency and megabytes of fork/exec memory overhead, invalidating micro-benchmark results.

### Source
- macOS Mach Kernel Headers: `/usr/include/mach/task_info.h` (`mach_task_basic_info`, `MACH_TASK_BASIC_INFO`, `resident_size_max`)
- Linux Man Pages: `proc(5)` (`/proc/[pid]/statm`), `getrusage(2)` (`ru_maxrss` normalization)
- Existing Codebase: `Sources/TTZipCore/Platform/PlatformMemory.swift`, `Tests/TTZipTests/PlatformMemoryTests.swift`

---

## R002: Out-of-Tree Fixture Cache & Multi-Process Concurrency Coordination (`flock`)

### Decision
Implement `EnwikFixtureCacheManager` with POSIX advisory file locking (`flock`) and RAII closure patterns:
1. Locate centralized cache directory: `~/Library/Caches/com.ttzip.tests/fixtures/` on macOS (`~/.cache/ttzip/fixtures/` on Linux).
2. Acquire exclusive advisory lock via `flock(fd, LOCK_EX)` on `<fixture>.lock` with an `EINTR` retry loop.
3. Perform **Idempotency Double-Check**: Once the lock is acquired, verify if the target fixture file already exists and satisfies expected length and SHA-256 (in case a concurrent process finished download/extraction while this process was waiting).
4. If missing/invalid, perform streaming download to a unique PID-stamped temp file (`<fixture>.tmp.<pid>.<uuid>`).
5. Verify downloaded payload hash, decompress in-process using TTZip's native C/Swift decompression engine, and atomically publish via POSIX `rename(tmpPath, finalPath)`.
6. Release lock in `defer { flock(fd, LOCK_UN); close(fd) }`.

### Rationale
1. **Kernel-Managed Lifecycle & Deadlock Immunity**: `flock` locks are bound to the open file table entry in the kernel. If a process crashes, encounters an assertion failure, or receives `SIGKILL`, the kernel automatically cleans up the lock, eliminating permanent deadlocks across CI and test runs.
2. **Parallel Process Safety (`swift test --parallel`)**: When Swift Package Manager runs tests across parallel worker processes, concurrent attempts to prepare the 1GB `enwik9` fixture safely serialize. The first process downloads and decompresses; subsequent workers wake up, pass the double-check, and immediately proceed with zero redundant I/O.
3. **Atomic File Publication**: Staging to temporary files followed by atomic POSIX `rename` guarantees readers never observe half-written or corrupted states.

### Alternatives Considered
1. **POSIX Record Locking (`fcntl` with `F_SETLK`)**:
   *Why rejected*: `fcntl` locks have a critical POSIX defect: closing *any* file descriptor to that file path anywhere within the process automatically releases all `fcntl` locks held on that file, creating subtle race conditions in multi-threaded environments.
2. **Atomic Lockfile Creation (`open(O_CREAT | O_EXCL)`)**:
   *Why rejected*: If a runner is terminated abnormally, stale `.lock` files remain permanently on disk, causing all future CI test runs to fail or hang indefinitely until manual file deletion.
3. **Named POSIX Semaphores (`sem_open`)**:
   *Why rejected*: Semaphores persist in kernel memory across process termination and do not clean up automatically on crashes, causing permanent deadlocks.

### Source
- Darwin POSIX Man Pages: `flock(2)`, `open(2)`, `rename(2)`
- Apple Open Source Libc: `sys/file.h` (`LOCK_SH`, `LOCK_EX`, `LOCK_UN`)
- Codebase References: `Sources/TTZipCore/Platform/PlatformFileSystem.swift`, `Tests/TTZipTests/IsolatedTempSandbox.swift`

---

## R003: High-Throughput Deterministic XML Corpus Synthesis Architecture

### Decision
Implement `SyntheticXmlCorpusGenerator` using a **Seed-Indexed Chunk Synthesis Architecture**:
1. **Static MediaWiki XML Token Bank**: Pre-compile static immutable byte slices representing authentic MediaWiki XML fragments (page headers, revision timestamps, contributor metadata, Wiki templates `{{cite web}}`, infoboxes, categories).
2. **Deterministic Virtual History ($O(1)$ Memory Overhead)**:
   - Partition generation into fixed 64 KB page-aligned chunks.
   - For chunk index $i$, select seed via SplitMix64 PRNG:
     $$\text{chunk\_seed}(i) = \begin{cases} \text{PRNG}(i - \delta), & \text{with probability } P_{\text{repeat}} \\ \text{PRNG}(i), & \text{otherwise} \end{cases}$$
     where $\delta = \text{RepeatDistance} / \text{ChunkSize}$.
   - Because historical chunks are purely functional derivations of their seed index, past chunks at offset $pos - D$ are re-synthesized instantly on the fly with **0 bytes of historical sliding window buffer memory**.
3. **Direct Zero-Allocation Streaming**:
   - Allocate a single reusable 64 KB page-aligned buffer using `PlatformMemory.allocateAlignedPageBuffer`.
   - Directly write sequentially to disk or memory stream via POSIX `write()`.

### Rationale
1. **Extreme Generation Throughput (> 3500 MB/s)**: Eliminating `String` dynamic allocations, UTF-8 transcoding, and ARC references reduces the inner loop to raw memory indexing and `memcpy`, easily saturating APFS/NTFS write buffers at > 3500 MB/s.
2. **Controllable Long-Distance Stress Testing**: Repeat distance $\delta$ and recurrence probability $P$ can be tuned (e.g. 1MB, 4MB, 16MB, 32MB) to explicitly profile LZMA2 dictionary match finders and ZSTD LDM window parameters.
3. **Stream-First & Zero OOM**: A 1GB synthesis job uses only a single 64KB page buffer, maintaining rock-solid resident memory on low-resource runners.

### Alternatives Considered
1. **Foundation `XMLDocument` / `String(format:)` Formatting**:
   *Why rejected*: Dynamic string allocations cap throughput at ~30-80 MB/s (requiring > 20 seconds for 1GB) and produce hundreds of megabytes of garbage allocations.
2. **Full In-Memory 1GB Pre-buffering (`Data(count: 1GB)`)**:
   *Why rejected*: Violates Stream-First invariants, spikes process baseline memory by 1GB before the compression benchmark even starts, and triggers OOM on resource-constrained CI runners.
3. **Sliding Memory History Buffer (32MB RAM)**:
   *Why rejected*: Unnecessary heap allocation and cache thrashing. Seed-indexed derivation achieves exact repetitive byte patterns with zero past-history memory.

### Source
- MediaWiki XML Export Schema: MediaWiki Export Format Specification
- RFC 8878 (Zstandard Compression) & Long Distance Matching (LDM) Parameters
- Igor Pavlov LZMA SDK (`MatchFinder.c`, `Bt4.c`)
- Existing Codebase: `Tests/TTZipTests/TestFileGenerator.swift`, `Sources/TTZipCore/Platform/PlatformMemory.swift`
