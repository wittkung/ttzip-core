# Data Model: TTZip 专业归档能力领域实体与状态机规范

**Feature Branch**: `082-pro-software-gap-audit`  
**Status**: Completed  
**Author**: Antigravity Agent & CTO

---

## 1. 实体关系总览 (Entity-Relationship Overview)

```
┌───────────────────────────────┐               ┌───────────────────────────────┐
│     SmartExtractStrategy      │               │     SplitVolumeDescriptor     │
├───────────────────────────────┤               ├───────────────────────────────┤
│ + sourceArchiveURL: String    │               │ + baseArchiveURL: String      │
│ + destinationDirectory: String│               │ + targetFormat: ArchiveFormat │
│ + effectiveRootCount: Int     │               │ + volumeSizePreset: PresetType│
│ + singleRootName: String?     │               │ + customVolumeSizeBytes: Int64│
│ + resolutionMode: Mode        │               │ + activeVolumeIndex: Int      │
│ + postAction: PostActionConfig│               │ + totalVolumeCount: Int       │
└───────────────────────────────┘               └───────────────────────────────┘
                │                                               │
                ▼                                               ▼
┌───────────────────────────────┐               ┌───────────────────────────────┐
│      ExternalEditSession      │               │     RecoveryRecordPackage     │
├───────────────────────────────┤               ├───────────────────────────────┤
│ + sessionId: String (UUID)    │               │ + archiveURL: String          │
│ + sourceArchiveURL: String    │               │ + recoveryPercent: Double     │
│ + archiveEntryPath: String    │               │ + parityBlockSizeBytes: Int   │
│ + stagedFileURL: String       │               │ + totalDataSlices: Int        │
│ + initialContentHash: String  │               │ + totalParitySlices: Int      │
│ + sessionState: SessionState  │               │ + eccAlgorithm: ECCAlgorithm  │
│ + lastModifiedTimestamp: Int64│               │ + sliceHashTable: [SliceHash] │
└───────────────────────────────┘               └───────────────────────────────┘
                │                                               │
                ▼                                               ▼
┌───────────────────────────────┐               ┌───────────────────────────────┐
│     BiometricAuthContext      │               │   BenchmarkExecutionMetrics   │
├───────────────────────────────┤               ├───────────────────────────────┤
│ + authReason: String          │               │ + threadCount: Int            │
│ + policy: LAPolicyType        │               │ + dictionarySizeMB: Int       │
│ + allowPasscodeFallback: Bool │               │ + compressThroughputMBs: Double│
│ + authState: AuthState        │               │ + decompressThroughputMBs: Dbl│
│ + hardwareSupport: Hardware   │               │ + benchmarkMIPS: Double       │
└───────────────────────────────┘               └───────────────────────────────┘
```

---

## 2. 核心领域实体详细定义 (Detailed Entity Specifications)

### 2.1 SmartExtractStrategy (智能解压策略)
- **`sourceArchiveURL`**: `String` (必填，合法的 file:// URL 字符串)
- **`destinationDirectory`**: `String` (必填，解压目标父目录绝对路径)
- **`effectiveRootCount`**: `Int` (必填，清洗系统元数据后的有效顶层实体数量，$\ge 0$)
- **`singleRootName`**: `String?` (可选，当 `effectiveRootCount == 1` 时的唯一顶层条目名称)
- **`resolutionMode`**: `SmartExtractResolutionMode` (必填，枚举值：`directExtract`, `wrapInFolder`, `emptyArchive`)
- **`collisionPolicy`**: `CollisionPolicy` (必填，枚举值：`autoRenameNumbered`, `overwriteExisting`, `skipExisting`, `abortWithError`)
- **`postAction`**: `PostActionConfig` (必填，结构体对象)
  - `moveToTrashAfterExtract`: `Bool` (必填)
  - `revealInFinder`: `Bool` (必填)
  - `playCompletionSound`: `Bool` (必填)
  - `sendSystemNotification`: `Bool` (必填)

### 2.2 SplitVolumeDescriptor (分卷归档描述符)
- **`baseArchiveURL`**: `String` (必填，基础归档路径)
- **`targetFormat`**: `String` (必填，枚举值：`sevenZip`, `zip`, `tar`)
- **`volumeNamingConvention`**: `String` (必填，枚举值：`numericExtension` e.g. `.7z.001`, `pkwareSpanned` e.g. `.z01` / `.zip`)
- **`volumeSizeBytes`**: `Int64` (必填，单卷物理切片字节上限，$\ge 65536$)
- **`activeVolumeIndex`**: `Int` (必填，当前正在写入的分卷索引，1-based)
- **`totalVolumeCount`**: `Int` (必填，归档总分卷数，动态递增，$\ge 1$)
- **`bytesWrittenInActiveVolume`**: `Int64` (必填，当前卷已写入字节数，$\ge 0$)
- **`cleanIncompleteVolumesOnFailure`**: `Bool` (必填，故障时是否自动回收未完成切片)

### 2.3 ExternalEditSession (外部就地编辑会话)
- **`sessionId`**: `String` (必填，UUIDv4 格式字符串)
- **`sourceArchiveURL`**: `String` (必填，关联的归档路径)
- **`archiveEntryPath`**: `String` (必填，归档内条目的 POSIX 相对路径)
- **`stagedFileURL`**: `String` (必填，沙盒临时提取文件的绝对路径)
- **`initialContentHash`**: `String` (必填，提取时的 BLAKE3/SHA-256 哈希值，64 字符十六进制)
- **`currentContentHash`**: `String` (必填，最新捕获的文件哈希值，64 字符十六进制)
- **`sessionState`**: `String` (必填，枚举值：`staged`, `listening`, `detectedChange`, `synchronizing`, `synchronized`, `conflict`, `closed`)
- **`lastModifiedTimestamp`**: `Int64` (必填，文件物理 mtime 毫秒级时间戳)

### 2.4 RecoveryRecordPackage (前向纠错恢复记录)
- **`archiveURL`**: `String` (必填，受保护归档路径)
- **`recoveryPercent`**: `Double` (必填，恢复记录冗余比例，范围 $0.01 \le x \le 0.10$)
- **`parityBlockSizeBytes`**: `Int` (必填，切片块大小，范围 $4096 \le x \le 262144$)
- **`totalDataSlices`**: `Int` (必填，主数据切片数，范围 $1 \le N \le 32768$)
- **`totalParitySlices`**: `Int` (必填，RS-FEC 校验切片数，范围 $1 \le M \le 32768$)
- **`eccAlgorithm`**: `String` (必填，枚举值：`cauchyReedSolomonGF16`, `vandermondeGF16`)
- **`sliceHashTable`**: `[SliceChecksumEntry]` (必填，切片校验和数组)
  - `sliceIndex`: `Int` (必填)
  - `crc32`: `Int64` (必填)
  - `blake3`: `String` (必填，32 字符十六进制)

### 2.5 BiometricAuthContext (生物识别认证上下文)
- **`authReason`**: `String` (必填，展示给用户的认证提示文案)
- **`policy`**: `String` (必填，枚举值：`deviceOwnerAuthentication`, `deviceOwnerAuthenticationWithBiometrics`)
- **`allowPasscodeFallback`**: `Bool` (必填，是否允许系统密码兜底)
- **`authState`**: `String` (必填，枚举值：`idle`, `authenticating`, `authenticated`, `failed`, `lockedOut`, `canceled`)
- **`hardwareBiometryType`**: `String` (必填，枚举值：`touchID`, `appleWatch`, `none`)

---

## 3. 状态机与生命周期定义 (State Machines & Lifecycle)

### 3.1 ExternalEditSession 状态机流转

```
  [staged] ──(launchApp)──▶ [listening] ──(VNode write)──▶ [detectedChange]
                                 ▲                                │
                                 │                           (150ms debounce)
                                 │                                │
                                 │                                ▼
                           [synchronized] ◀──(replaceItemAt)── [synchronizing]
                                 │                                │
                           (sessionClose)                    (writeError)
                                 │                                │
                                 ▼                                ▼
                              [closed] ◀───────────────────── [conflict]
```

### 3.2 RecoveryRecord 修复生命周期流转

```
  [Archive Corrupted] ──▶ [Scan Universal Trailer] ──▶ [Verify Slice Hashes]
                                                             │
                                                     (Corrupted Slices <= M)
                                                             │
                                                             ▼
  [Archive Repaired] ◀── [Atomic Overwrite] ◀── [CRS Matrix Solve & Rebuild]
```
