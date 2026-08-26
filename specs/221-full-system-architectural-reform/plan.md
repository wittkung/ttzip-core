# Plan: Full System Architectural Reform & Defect Remediation

**Feature**: `221-full-system-architectural-reform`  
**Status**: Ready for Implementation  

---

## 1. Technical Architecture & Component Structure

```
TTZip Pro Architecture
├── Layer 0: Safe Rust Core Engine (rust/ttzip-engine & rust/ttzip-glue)
│   ├── Unified Fast-Path Dispatcher (ZIP, TAR, 7z, libarchive fallback)
│   ├── Bounded Channel Paged Streaming Compression
│   ├── Batch Selective Stream Extractor
│   ├── VirtualMultiVolumeReader Zero-Disk Inspection
│   └── Benchmark Module Consolidation
├── Layer 1: C Bridge & Interop ABI (Sources/CTTZipBridge)
│   └── Standardized C-ABI Headers (ttzip_rust_glue.h)
├── Layer 2: Swift 6 Core Engine (Sources/TTZipCore)
│   ├── Write-Ahead Journaling (DifferentialExtractTransaction)
│   ├── APFS clonefile Staging & Atomic Swap (AtomicCompressTransaction)
│   ├── Zero-Heap-Residue SecureBytes
│   ├── Zero-Lock ArchiveEntry with ArchiveMimeMapper
│   ├── Persistent RustVfsSession
│   └── NativeComputeDispatcher (Swift 6 Concurrency Isolation)
└── Layer 3: Presentation & Tools (Sources/TTZipApp)
    ├── ArchiveOutlineItem Reference Adapter for NSOutlineView
    └── Instant Reactive VFS Search
```

---

## 2. File Modification & Creation Inventory

1. `rust/ttzip-engine/src/archive/unified/create.rs` [MODIFY]
2. `rust/ttzip-engine/src/archive/unified/extract.rs` [MODIFY]
3. `rust/ttzip-engine/src/archive/unified/extract_single.rs` [MODIFY]
4. `rust/ttzip-engine/src/archive/unified/inspect.rs` [MODIFY]
5. `rust/ttzip-engine/src/zip/writer/mod.rs` & `parallel.rs` [MODIFY]
6. `rust/ttzip-engine/src/lib.rs` & `rust/ttzip-glue/src/lib.rs` [MODIFY]
7. `rust/ttzip-glue/Cargo.toml` [MODIFY]
8. `Sources/CTTZipBridge/include/ttzip_rust_glue.h` [MODIFY]
9. `Sources/TTZipCore/Commands/ExtractCommand.swift` [MODIFY]
10. `Sources/TTZipCore/Commands/CompressCommand.swift` [MODIFY]
11. `Sources/TTZipCore/ArchiveExtractor.swift` [MODIFY]
12. `Sources/TTZipCore/ArchiveReader.swift` [MODIFY]
13. `Sources/TTZipCore/ArchiveEntry.swift` [MODIFY]
14. `Sources/TTZipCore/Types/ArchiveEntryMetadata.swift` [MODIFY]
15. `Sources/TTZipCore/Security/SecureBytes.swift` [MODIFY]
16. `Sources/TTZipCore/Platform/PlatformHardware.swift` [MODIFY]
17. `Sources/TTZipCore/Bridge/RustVfsSession.swift` [NEW]
18. `Sources/TTZipCore/Bridge/RustVfsBridge.swift` [MODIFY]
19. `Sources/TTZipCore/Concurrency/NativeComputeDispatcher.swift` [NEW]
20. `Sources/TTZipCore/Concurrency/ConcurrencyBridge.swift` [NEW]
21. `Sources/TTZipCore/Memory/MemoryPagePool.swift` [DELETE]
22. `Sources/TTZipApp/Views/Explorer/ArchiveOutlineItem.swift` [NEW]
23. `Sources/TTZipApp/Views/Explorer/NativeArchiveOutlineView+Delegates.swift` [MODIFY]
24. `Sources/TTZipApp/ViewModels/ArchiveTreeStore.swift` [MODIFY]
