# Contract: Global Background Operations Queue & Telemetry Hub

- **Version**: 1.0.0
- **Scope**: Desktop UI <-> Background Task Queue <-> Dock & MenuBar Telemetry

---

## 1. Interface Signatures

```swift
@MainActor
public protocol ArchiveOperationsQueueCentering: AnyObject {
    var tasks: [QueuedArchiveOperation] { get }
    var activeTasksCount: Int { get }
    var overallProgress: Double { get }
    var overallThroughputMBs: Double { get }
    
    func enqueue(operation: QueuedArchiveOperation)
    func updateProgress(id: UUID, bytesProcessed: Int64, totalBytes: Int64, currentEntry: String, throughputMBs: Double)
    func markCompleted(id: UUID)
    func markFailed(id: UUID, error: Error)
    func cancel(id: UUID)
}
```

## 2. Invariants & Guardrails
1. The operations queue center MUST be a singleton isolated on `@MainActor`.
2. All long-running operations across the app MUST register with the center.
3. Dock icon progress bar MUST accurately display the aggregate progress across all active tasks.
