# Feature Specification: 148-frontend-c-wiring-and-swift-slimming

## 1. Executive Summary & User Scenarios

### User Scenario 1 (US1): High-Speed Natural Sorting in Disk & Archive Views
As a user browsing folders and archives containing thousands of numerical files (`img_1.png`, `img_2.png`, `img_10.png`), I want instant alphabetical and numerical natural sorting powered by `NativeMicrokernelBridge.naturalCompare` in pure C11 without Swift NSString bridging overhead.

### User Scenario 2 (US2): Sub-Nanosecond Header Sniffing & In-Memory Preview in Media Views
As a user previewing images, audio, video, or documents inside an archive, I want `MediaPreviewFactory` to sniff file magic numbers directly from file headers via `NativeMicrokernelBridge.sniffMagic` and extract preview streams directly into RAM via `NativeMicrokernelBridge.extractEntryToMemory` without creating temporary files on disk.

### User Scenario 3 (US3): Sub-Millisecond Search Filtering in Archive Tree Store
As a user filtering 10,000+ entries in the archive explorer, I want `ArchiveTreeStore` search matching to leverage fast substring and Radix tree search, delivering sub-millisecond search results on every keystroke.

---

## 2. Functional Requirements

- **FR-001**: `Sources/TTZipApp/Services/DiskItemSorter.swift` must use `NativeMicrokernelBridge.naturalCompare` (`ttzip_strnatcasecmp`) for all `.nameAsc` and `.nameDesc` comparisons.
- **FR-002**: `Sources/TTZipApp/Services/MediaPreviewFactory.swift` must integrate `NativeMicrokernelBridge.sniffMagic` and `NativeMicrokernelBridge.extractEntryToMemory` for memory-mapped instant previews.
- **FR-003**: `Sources/TTZipApp/ViewModels/ArchiveTreeStore.swift` must optimize search filtering by leveraging C11 string compare and search fast-paths.
- **FR-004**: All existing 76+ unit and matrix tests must pass 100% green in `./scripts/local-ci.sh`.
- **FR-005**: Maintain zero Apple GCD calls in core business logic.

---

## 3. Success Criteria

1. **Sorting Throughput**: Natural sorting in `DiskItemSorter` executes at >30 Million ops/s without UI frame drops.
2. **Instant Preview**: Previews for supported archive entries load with 0 temporary disk writes.
3. **Zero Regression**: 100% of Swift unit tests pass green locally.
