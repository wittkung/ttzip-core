# Data Model: Compression Formats and Algorithms Architecture

**Feature**: `138-compression-formats-algorithms`  
**Date**: 2026-08-20  
**Phase**: Phase 1 Design

---

## 1. Domain Entities Taxonomy

This data model defines the formal schema, properties, types, and invariants governing TTZip's format registry, underlying compression algorithms, match finding strategies, entropy coders, hardware acceleration bindings, and benchmark performance metrics.

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                               FormatAlgorithmMatrix                                    │
│   ├── formats: Map<FormatId, ArchiveContainerDescriptor>                               │
│   ├── algorithms: Map<AlgorithmId, CompressionAlgorithmDescriptor>                     │
│   ├── hardwareKernels: Map<KernelId, HardwareKernelDescriptor>                         │
│   └── benchmarkProfiles: Array<BenchmarkWorkloadProfile>                               │
└──────────────────────────────────────────┬─────────────────────────────────────────────┘
                                           │ references
               ┌───────────────────────────┴───────────────────────────┐
               ▼                                                       ▼
┌──────────────────────────────┐                       ┌──────────────────────────────┐
│  ArchiveContainerDescriptor  │                       │CompressionAlgorithmDescriptor│
│  - formatId: FormatId        │                       │  - algorithmId: AlgorithmId  │
│  - displayName: String       │                       │  - formalSpec: String        │
│  - extensions: Array<String> │                       │  - algorithmFamily: Family   │
│  - containerParadigm: Enum   │                       │  - matchFinder: MatchFinder  │
│  - primaryHeaderAnchor: Enum │                       │  - entropyCoder: EntropyCoder│
│  - defaultAlgorithm: String  │                       │  - windowBounds: WindowRange │
│  - allowedAlgorithms: Array  │                       │  - lengthBounds: LengthRange │
│  - supportedLevels: Array    │                       │  - computationalComplexity:  │
│  - encryptionMethods: Array  │                       │  - hardwareAcceleration:     │
│  - streamingSupport: Boolean │                       │  - primaryUseCases: Array    │
│  - nativeCEngine: String     │                       └──────────────────────────────┘
└──────────────────────────────┘
```

---

## 2. Entity Specifications

### 2.1 `ArchiveContainerDescriptor`
Represents the structural specification and physical capabilities of an archive container format.

| Field Name | Type | Required | Constraints / Enums | Description |
| :--- | :--- | :---: | :--- | :--- |
| `formatId` | `String` | Yes | Enum: `sevenZip`, `zip`, `tar`, `zst`, `gz`, `bz2`, `xz`, `lzip`, `lz4`, `brotli`, `lrzip`, `aar`, `snappy`, `wim`, `dmg`, `iso`, `rar`, `cab`, `cpio`, `xar` | Unique programmatic identifier of the format. |
| `displayName` | `String` | Yes | Non-empty string (e.g., `"7Z"`, `"ZIP"`, `"TAR.ZST"`) | Human-readable format name for UI and CLI display. |
| `primaryExtension` | `String` | Yes | Starts with `.` (e.g., `".7z"`, `".zip"`, `".tar.zst"`) | Canonical default file extension. |
| `compatibleExtensions` | `Array<String>` | Yes | Array of extension strings (e.g., `[".tgz", ".tar.gz"]`) | Complete set of file extensions mapped to this format. |
| `containerParadigm` | `String` | Yes | Enum: `"randomAccessSeekable"`, `"sequentialStreaming"`, `"sectorIndexedDiskImage"` | Architectural stream and directory traversal layout. |
| `directoryIndexLocation` | `String` | Yes | Enum: `"endOfFileTrailer"`, `"interleavedBlockHeaders"`, `"startHeaderOffset"`, `"xmlResourceTable"`, `"opticalVolumeDescriptors"` | Physical location of the table of contents / file index. |
| `defaultAlgorithm` | `String` | Yes | Valid `AlgorithmId` (e.g., `"lzma2"`, `"deflate"`, `"zstd"`, `"none"`) | Default compression algorithm selected on archive creation. |
| `allowedAlgorithms` | `Array<String>` | Yes | Array of valid `AlgorithmId` strings | Set of algorithms supported within this container format. |
| `supportedCompressionLevels`| `Array<Integer>`| Yes | Integers within range `[-5, 22]` | List of supported compression level integers. |
| `supportedEncryptionMethods`| `Array<String>` | Yes | Items from: `"aes256Cbc"`, `"aes256CtrWinZip"`, `"aes128CtrWinZip"`, `"zipCrypto"`, `"dmgAppleEncrypted"`, `"none"` | Cryptographic ciphers supported by the format. |
| `supportsSolidArchiving` | `Boolean` | Yes | Boolean | Whether multiple files can be compressed into a single shared solid stream. |
| `supportsSplitVolumes` | `Boolean` | Yes | Boolean | Whether multi-part split volumes (`.001`, `.z01`) are supported. |
| `supportsHeaderEncryption`| `Boolean` | Yes | Boolean | Whether directory filenames and sizes can be encrypted (`mhe=on`). |
| `supportsUnixPermissions` | `Boolean` | Yes | Boolean | Whether POSIX UID, GID, and octal permissions (`0o755`) are preserved. |
| `supportsExtendedAttributes`| `Boolean` | Yes | Boolean | Whether macOS Extended Attributes (`xattr`) and ACLs are supported. |
| `nativeCEngine` | `String` | Yes | Non-empty string (e.g., `"libdeflate + SIMD C"`, `"LZMA SDK + fast-lzma2"`, `"libzstd v1.5.6"`) | Underlying static C engine implementation. |

---

### 2.2 `CompressionAlgorithmDescriptor`
Represents the mathematical formulation, dictionary models, and complexity bounds of an underlying compression algorithm.

| Field Name | Type | Required | Constraints / Enums | Description |
| :--- | :--- | :---: | :--- | :--- |
| `algorithmId` | `String` | Yes | Enum: `"deflate"`, `"lzma"`, `"lzma2"`, `"zstd"`, `"bzip2"`, `"lz4"`, `"lz4hc"`, `"brotli"`, `"lzfse"`, `"snappy"`, `"lrzip"`, `"lzx"`, `"xpress"`, `"ppmd"`, `"store"` | Unique programmatic identifier of the algorithm. |
| `displayName` | `String` | Yes | Non-empty string (e.g., `"Deflate (RFC 1951)"`, `"LZMA2"`, `"Zstandard (RFC 8878)"`) | Full technical name of the algorithm. |
| `governingSpecification`| `String` | Yes | Non-empty string (e.g., `"IETF RFC 1951"`, `"IETF RFC 8878"`, `"7-Zip LZMA SDK"`) | Authoritative RFC or published specification reference. |
| `algorithmFamily` | `String` | Yes | Enum: `"dictionaryLz77"`, `"markovRangeCoding"`, `"finiteStateEntropy"`, `"blockSortingBwt"`, `"statisticalContextModeling"`, `"rawByteAligned"`, `"uncompressedPassthrough"` | Theoretical compression algorithm family. |
| `matchFinderStrategy` | `MatchFinderConfig` | Yes | Structured Object (defined below) | Sliding window match-finding mechanics. |
| `entropyCoderStrategy` | `EntropyCoderConfig`| Yes | Structured Object (defined below) | Statistical entropy and symbol encoding model. |
| `minWindowSizeBytes` | `Integer` | Yes | $\ge 0$ | Minimum sliding window size in bytes. |
| `maxWindowSizeBytes` | `Integer` | Yes | $\ge 0$ | Maximum sliding window size in bytes. |
| `minMatchLengthBytes` | `Integer` | Yes | $\ge 0$ | Minimum compressible match length in bytes. |
| `maxMatchLengthBytes` | `Integer` | Yes | $\ge 0$ | Maximum single-token match length in bytes. |
| `compressionSpeedRating`| `String` | Yes | Enum: `"ultraFast"`, `"fast"`, `"medium"`, `"slow"`, `"ultraSlow"` | Relative operational compression throughput category. |
| `decompressionSpeedRating`| `String` | Yes | Enum: `"extremeWireSpeed"`, `"veryHigh"`, `"high"`, `"medium"`, `"moderate"` | Relative operational decompression throughput category. |
| `memoryComplexityClass` | `String` | Yes | Enum: `"constantLowBounded"`, `"windowBounded"`, `"heapScaleBounded"`, `"ramScaleIntensive"` | Memory allocation complexity profile during execution. |
| `hardwareVectorPaths` | `Array<String>` | Yes | Array of `KernelId` strings | Apple Silicon SIMD kernels used on hot paths. |

---

### 2.3 `MatchFinderConfig`
Describes the pattern discovery and substring lookup data structures.

| Field Name | Type | Required | Constraints / Enums | Description |
| :--- | :--- | :---: | :--- | :--- |
| `type` | `String` | Yes | Enum: `"hashChain"`, `"directBuckets"`, `"binaryTree"`, `"patriciaTrie"`, `"rollingHashRzip"`, `"swarWordMatching"`, `"neonVectorMatch"`, `"suffixSortBwt"`, `"none"` | Match finder data structure or search heuristic. |
| `hashBytes` | `Integer` | Yes | Range: `[0, 8]` | Number of prefix bytes used for hash indexing (e.g., 3, 4, 8). |
| `parsingHeuristic` | `String` | Yes | Enum: `"greedy"`, `"lazyEvaluation"`, `"twoStepLazy"`, `"optimalForwardDp"`, `"zopfliIterativeEntropy"`, `"contextStatistical"`, `"none"` | Match selection and cost-optimization strategy. |

---

### 2.4 `EntropyCoderConfig`
Describes the statistical symbol coding model transforming tokens into compressed bitstreams.

| Field Name | Type | Required | Constraints / Enums | Description |
| :--- | :--- | :---: | :--- | :--- |
| `type` | `String` | Yes | Enum: `"canonicalHuffman"`, `"finiteStateEntropyANS"`, `"binaryArithmeticRangeCoder"`, `"subbotinRangeCoder"`, `"noneRawBytes"` | Entropy coding mathematical model. |
| `fractionalBitsSupport` | `Boolean` | Yes | Boolean | Whether sub-bit fractional symbol probabilities ($< 1.0\text{ bit}$) are encoded. |
| `interleavedStreamCount` | `Integer` | Yes | Range: `[0, 8]` | Number of concurrent interleaved bitstreams/states (e.g. 4 for Zstd/LZFSE). |
| `staticDictionaryAvailable`| `Boolean` | Yes | Boolean | Whether pre-defined static dictionaries are built-in (e.g. Brotli 120KB). |

---

### 2.5 `HardwareKernelDescriptor`
Describes Apple Silicon hardware SIMD vector and cryptographic micro-kernels.

| Field Name | Type | Required | Constraints / Enums | Description |
| :--- | :--- | :---: | :--- | :--- |
| `kernelId` | `String` | Yes | Enum: `"arm64PmullCrc64"`, `"armv8AcleCrc32"`, `"armNeonAdler32DotProd"`, `"armv8Aes256Interleaved"`, `"armNeonSwarMatch"`, `"armNeonByteShuffle"`, `"armNeonBitGroom"` | Unique kernel identifier. |
| `instructionSetFeatures`| `Array<String>` | Yes | Items from: `"FEAT_PMULL"`, `"FEAT_CRC32"`, `"FEAT_DOTPROD"`, `"FEAT_AES"`, `"FEAT_SHA256"`, `"FEAT_NEON"` | Required ARM CPU capability feature flags. |
| `vectorRegisterWidthBits`| `Integer` | Yes | Range: `[64, 512]` | SIMD register width utilized (e.g., 128 bits for ARM NEON). |
| `unrollFactor` | `Integer` | Yes | Range: `[1, 16]` | Loop unrolling depth per iteration (e.g., 4-way, 8-way, 12-way). |
| `peakThroughputMBps` | `Double` | Yes | Positive floating-point value | Measured peak physical throughput on Apple Silicon hardware. |
| `cBridgeSourceFile` | `String` | Yes | Non-empty string path | C bridge implementation file path in TTZip repository. |

---

### 2.6 `BenchmarkWorkloadProfile`
Describes empirical benchmark measurements and Pareto trade-offs across industrial workloads.

| Field Name | Type | Required | Constraints / Enums | Description |
| :--- | :--- | :---: | :--- | :--- |
| `workloadId` | `String` | Yes | Enum: `"massiveSmallFiles"`, `"structuredLogText"`, `"highEntropyBinary"`, `"largeDataBlock500MB"` | Industrial workload test scenario. |
| `formatId` | `String` | Yes | Valid `FormatId` | Archive format evaluated. |
| `compressionLevel` | `Integer` | Yes | Integer in range `[-5, 22]` | Compression level evaluated. |
| `encryptionEnabled` | `Boolean` | Yes | Boolean | Whether AES-256 encryption was active during benchmark. |
| `packagingThroughputMBps`| `Double` | Yes | $\ge 0.0$ | Measured packaging/compression speed in MB/s. |
| `extractionThroughputMBps`| `Double` | Yes | $\ge 0.0$ | Measured extraction/decompression speed in MB/s. |
| `compressionRatioPercent`| `Double` | Yes | $0.0 \le \text{ratio} \le 150.0$ | Compressed size as a percentage of original size. |
| `peakMemoryOccupancyMB` | `Double` | Yes | $\ge 0.0$ | Peak resident memory footprint in megabytes during operation. |
