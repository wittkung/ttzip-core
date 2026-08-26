# Data Model: 095-comprehensive-systems-engineering-evolution

## Core Data Models & Type Invariants

### 1. `TTZipStructHeader` (C Defensive Invariant Model)
```c
typedef struct {
    uint32_t magic;           // 0x545A4354 ("TZCT") or format-specific magic
    uint32_t flags;           // State flags (INITIALIZED, LOCKED, STREAMING)
    uint64_t allocated_bytes; // Bounded capacity tracker
    void* internal_state;     // Type-erased engine payload
} ttzip_struct_header_t;
```

### 2. `MultiWayConsensusReport` (Testing & Verification Model)
```swift
public struct MultiWayConsensusReport: Sendable, Codable {
    public let archivePath: String
    public let format: String
    public let oraclesEvaluated: [String] // ["bsdtar", "ditto", "7z", "zipinfo"]
    public let matchRate: Double          // 1.0 = 100% consensus
    public let byteIdenticalEntries: Int
    public let divergentEntries: [String]
    public let isConsensusPassed: Bool
}
```

### 3. `PropertyTreeConfiguration` (Generative Fuzzing Model)
```swift
public struct PropertyTreeConfiguration: Sendable, Codable {
    public let maxDepth: Int             // e.g. 25
    public let fileCount: Int            // e.g. 500
    public let unicodeNormalizationMode: String // "nfc", "nfd", "mixed"
    public let includeSparseFiles: Bool
    public let includeSymlinks: Bool
    public let entropySpectrum: String   // "zero", "low", "compressible", "random"
}
```

### 4. `SIMDSearchBlockDescriptor` (Vector Filtering Model)
```swift
public struct SIMDSearchBlockDescriptor: Sendable {
    public let flatPathBufferOffset: UInt32
    public let pathLength: UInt16
    public let firstCharacter: UInt8
    public let lastCharacter: UInt8
    public let itemIndex: UInt32
}
```
