# Data Model: C-Blosc2 Exhaustive Architectural Absorption (Feature 094)

## 一、 核心实体与类型模型 (Data Structures & Entities)

### 1. BloscLZ Native Codec Models (C & Swift)

```c
typedef struct {
    uint8_t clevel;             // Compression level 1..9
    uint8_t hash_log;           // Hash table bit width (12..14)
    uint32_t typesize;          // Element type size in bytes (1, 2, 4, 8)
    bool use_bitshuffle;        // Pre-stage bit-level SIMD transposition
} ttzip_blosclz_config_t;

typedef struct {
    size_t uncompressed_bytes;  // Original source byte count
    size_t compressed_bytes;    // Compressed payload byte count
    double compression_ratio;   // uncompressed / compressed
    double throughput_mbs;      // Processing speed in MB/s
} ttzip_codec_metrics_t;
```

```swift
public struct BloscLZConfiguration: Sendable, Codable, Equatable {
    public let level: Int               // 1..9
    public let hashLog: Int             // 12..14 (default: 14)
    public let elementSize: Int         // 1, 2, 4, 8
    public let useBitShuffle: Bool      // Whether BitShuffle is prepended
    
    public init(level: Int = 5, hashLog: Int = 14, elementSize: Int = 4, useBitShuffle: Bool = true) {
        self.level = max(1, min(9, level))
        self.hashLog = max(12, min(14, hashLog))
        self.elementSize = elementSize
        self.useBitShuffle = useBitShuffle
    }
}
```

---

### 2. N-Dimensional Tensor & Hypercube Chunker Models (`b2nd`)

```swift
public struct NDimTensorShape: Sendable, Codable, Equatable {
    public let dimensions: [Int64]      // Global shape e.g. [1024, 1024, 64]
    public let chunkShape: [Int64]      // L3/SLC chunk shape e.g. [256, 256, 16]
    public let blockShape: [Int64]      // L2 block shape e.g. [32, 32, 8]
    public let dataType: String         // e.g. "<f4", "<f8", "<i4"
    public let elementByteSize: Int     // e.g. 4 for Float32
    
    public var rank: Int { dimensions.count }
    public var totalElements: Int64 { dimensions.reduce(1, *) }
    public var totalBytes: Int64 { totalElements * Int64(elementByteSize) }
}

public struct NDimSliceCoordinateRange: Sendable, Codable, Equatable {
    public let startIndices: [Int64]    // e.g. [0, 50, 0]
    public let endIndices: [Int64]      // e.g. [100, 150, 10]
    public let strides: [Int64]         // e.g. [1, 1, 1]
    
    public init(start: [Int64], end: [Int64], strides: [Int64]? = nil) {
        self.startIndices = start
        self.endIndices = end
        self.strides = strides ?? Array(repeating: 1, count: start.count)
    }
}

public struct NDimIntersectingBlock: Sendable, Codable, Equatable {
    public let chunkIndex: Int64
    public let blockIndexInChunk: Int32
    public let chunkOffsetInFile: Int64
    public let blockOffsetInChunk: Int32
    public let compressedBytes: Int32
    public let uncompressedBytes: Int32
}
```

---

### 3. Context Memory Pool Models (C & Swift)

```c
typedef struct {
    void* raw_buffer;           // Base allocation pointer
    uint8_t* aligned_buffer;    // 64-byte or 16KB aligned working pointer
    size_t capacity;            // Allocated capacity
    size_t in_use;              // Active byte cursor
    bool is_locked;             // Atomic in-use flag
} ttzip_thread_scratchpad_t;

typedef struct {
    size_t worker_count;
    size_t scratchpad_size;
    ttzip_thread_scratchpad_t* scratchpads;
} ttzip_context_pool_t;
```

```swift
public final class ThreadLocalContextMemoryPool: @unchecked Sendable {
    public static let shared = ThreadLocalContextMemoryPool()
    
    public struct Statistics: Sendable, Codable, Equatable {
        public let totalScratchpads: Int
        public let scratchpadCapacityBytes: Int
        public let totalAllocatedBytes: Int
        public let activeLeases: Int
        public let zeroAllocHitRate: Double
    }
}
```
