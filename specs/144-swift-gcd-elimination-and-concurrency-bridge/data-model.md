# Data Model & Concurrency Entities: 144-swift-gcd-elimination-and-concurrency-bridge

## 1. Concurrency Entities

### `ConcurrencyBridge` (Enum Namespace)
- **Type**: `public enum ConcurrencyBridge`
- **Responsibilities**: Global namespace for cross-platform parallel execution, hardware CPU topology queries, and system memory budgeting.

### `ParallelForBox` (Internal Box Class)
- **Type**: `final class ParallelForBox: @unchecked Sendable`
- **Fields**:
  - `worker: @Sendable (Int) -> Void` (Immutable worker closure)
- **Invariants**: Allocated exactly once per `parallelFor` call; deallocated strictly after the C barrier completes.

### `ThreadBudget` (Sub-Namespace)
- **Type**: `public enum ConcurrencyBridge.ThreadBudget`
- **Operations**:
  - `optimalThreadCount(for requestedThreads: Int) -> Int`
  - `setOverride(maxThreads: Int)`
  - `topology: ttzip_cpu_topology_t` (Properties: `total_logical_cores: UInt32`, `p_cores: UInt32`, `e_cores: UInt32`, `default_threads: UInt32`)

### `MemoryBudget` (Sub-Namespace)
- **Type**: `public enum ConcurrencyBridge.MemoryBudget`
- **Operations**:
  - `safeBudget: UInt64` (Safe memory allocation limit in bytes)
  - `query() -> ttzip_mem_budget_t` (Properties: `total_physical_ram: UInt64`, `available_physical_ram: UInt64`, `safe_budget_bytes: UInt64`)
  - `clamp(desiredBytes: UInt64, minBytes: UInt64, maxBytes: UInt64) -> UInt64`
  - `setOverride(maxBudgetBytes: UInt64)`

## 2. Updated Observer Entities

### `ArchiveProgressObserverProtocol`
- **Type**: `public protocol ArchiveProgressObserverProtocol: AnyObject, Sendable`
- **Updated Interface**:
  - `func onProgressUpdate(percentage: Double, processedBytes: Int64, totalBytes: Int64)`
  - Removal of `dispatchQueue: DispatchQueue?` parameter.

### `WeakObserverWrapper`
- **Type**: `public final class WeakObserverWrapper: @unchecked Sendable`
- **Fields**:
  - `weak var observer: AnyObject?`
  - `let queueLabel: String?` (Replaces `DispatchQueue?` with pure metadata label if needed)
