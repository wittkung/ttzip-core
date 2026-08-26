# Data Model: 080-test-suite-acceleration-and-optimization

## Entities

### 1. TestSuiteExecutionConfig
Represents the runtime configuration and profiling mode for test suite execution.

```typescript
interface TestSuiteExecutionConfig {
  mode: "default" | "deepFuzz" | "fullBenchmark";
  isDeepFuzzEnabled: boolean;           // Derived from TTZIP_DEEP_FUZZ != nil
  isBenchmarkModeEnabled: boolean;      // Derived from TTZIP_RUN_BENCHMARKS != nil
  maxConcurrentTasks: number;           // Defaults to ProcessInfo.processInfo.activeProcessorCount
  defaultFuzzIterationsPerFormat: number; // 10 in default mode, 200 in deepFuzz
  matrixFuzzIterationsPerFormat: number;  // 10 in default mode, 1000 in deepFuzz
}
```

### 2. FuzzParallelTask
Represents an isolated, parallel mutation fuzz task dispatched to a worker thread.

```typescript
interface FuzzParallelTask {
  format: "zip" | "sevenZip" | "tar" | "zstd" | "gzip";
  iterationIndex: number;
  deterministicSeed: string;            // Hex uint64 derived from master seed
  mutationOperator: "bitFlip" | "byteShuffle" | "truncateStream" | "oversizeHeader" | "corruptCRC" | "invalidDictSize" | "corruptMagic" | "injectZipSlipPath";
  inMemorySourceBytes: number;
  expectedDefenseResult: "errorRejected" | "safeDecompressed";
  actualStatus: number;                 // ttzip_error_t or ArchiveReader outcome
  isMemorySafe: boolean;                // Zero SIGSEGV, zero buffer overflow
}
```

### 3. AdaptiveBenchmarkProfile
Defines the iteration scale and payload parameters for performance benchmark tests.

```typescript
interface AdaptiveBenchmarkProfile {
  benchmarkName: string;
  payloadSizes: number[];               // e.g. [65536, 1048576, 10485760, 52428800]
  defaultIterations: number[];          // e.g. [50, 10, 2, 1]
  benchmarkModeIterations: number[];   // e.g. [2000, 500, 100, 20]
  activeIterations: number[];           // Resolved based on isBenchmarkModeEnabled
  throughputFloorMBs: number;           // Minimum required throughput threshold
}
```

### 4. CancellationSyncState
Encapsulates structured synchronization state for concurrent task cancellation tests.

```typescript
interface CancellationSyncState {
  totalTasks: number;                   // 100 tasks
  cancelledTasks: number;               // Tasks designated for early cancellation (e.g. i % 3 == 0)
  completedTasks: number;               // Tasks that completed search
  allCleanedUp: boolean;                // Invariant: all child TaskGroups exited cleanly
  elapsedMilliseconds: number;          // Must be <= 50ms in optimized test
}
```
