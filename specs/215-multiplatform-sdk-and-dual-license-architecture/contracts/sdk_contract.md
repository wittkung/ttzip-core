# Interface Contract: Multi-Language SDK Bindings

**Feature**: `215-multiplatform-sdk-and-dual-license-architecture`  
**Status**: `FROZEN` (Swift 6 SDK in MVP; Python/Node/Java in Milestone 2)  

---

## 1. Swift 6 SDK Contract (`TTZipCore`) [MVP / Phase 3]

```swift
import Foundation
import CTTZipBridge

public struct ArchiveProgress: Sendable {
    public let bytesProcessed: UInt64
    public let bytesTotal: UInt64
    public let currentFile: String
    public let fractionCompleted: Double
    public let throughputMBPerSec: Double
}

public actor ArchiveExtractor {
    public init() {}
    
    public func extract(
        from archiveURL: URL,
        to destinationURL: URL,
        password: String? = nil
    ) -> AsyncThrowingStream<ArchiveProgress, Error> {
        AsyncThrowingStream { continuation in
            var options = TTZipExtractOptions(
                destination_path: destinationURL.path,
                password: password,
                thread_budget: 0,
                overwrite_existing: true,
                preserve_permissions: true,
                dry_run: false,
                progress_callback: { processed, total, current, udata in
                    guard let udata = udata else { return true }
                    let box = Unmanaged<ContinuationBox>.fromOpaque(udata).takeUnretainedValue()
                    box.continuation.yield(ArchiveProgress(
                        bytesProcessed: processed,
                        bytesTotal: total,
                        currentFile: current.map { String(cString: $0) } ?? "",
                        fractionCompleted: total > 0 ? Double(processed) / Double(total) : 0,
                        throughputMBPerSec: 0
                    ))
                    return !box.isCancelled
                },
                user_data: nil
            )
            // Execute extraction via ttzip_rust_archive_extract_unified
        }
    }
}
```

---

## 2. Python SDK Contract (`ttzip`) [Milestone 2 - PyO3 Direct Binding]

Architecture: Python imports `ttzip` (a native Rust extension built with PyO3 targeting `ttzip-engine` directly, bypassing intermediate C-ABI).

```python
from typing import Callable, Optional, List
from dataclasses import dataclass

@dataclass
class ProgressEvent:
    bytes_processed: int
    bytes_total: int
    current_file: str
    ratio: float
    speed_mb_s: float

def compress(
    sources: List[str],
    output: str,
    format: str = "auto",
    level: int = 6,
    password: Optional[str] = None,
    progress_callback: Optional[Callable[[ProgressEvent], bool]] = None
) -> None:
    """Compress files using native Rust ttzip-engine."""
    ...

def extract(
    archive: str,
    destination: str,
    password: Optional[str] = None,
    progress_callback: Optional[Callable[[ProgressEvent], bool]] = None
) -> None:
    """Extract archive safely preventing Zip Slip."""
    ...
```

---

## 3. Node.js & Java/Kotlin Contracts [Milestone 2]

- **Node.js**: `@ttzip/core` compiled via `napi-rs` directly against `ttzip-engine`.
- **Java 22+**: `com.ttzip:ttzip-core` bound via `java.lang.foreign` (FFM API) to `libttzip.so` / `libttzip.dylib` / `ttzip.dll`.
