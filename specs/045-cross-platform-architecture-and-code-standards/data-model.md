# Data Model: 045-cross-platform-architecture-and-code-standards

本数据模型严格遵循 Swift 6 严格并发（Strict Concurrency）与零通配（Zero Bare Objects）规范，为跨平台抽象层 (PAL) 提供强类型契约。

---

## 一、 实体定义 (Entities)

### 1. `PlatformOperatingSystem` (操作系统环境标识)

```swift
public enum PlatformOperatingSystem: String, Sendable, Codable, CaseIterable {
    case macOS = "macOS"
    case windows = "Windows"
    case linux = "Linux"
    case unknown = "Unknown"
    
    public static var current: PlatformOperatingSystem {
        #if os(macOS)
        return .macOS
        #elseif os(Windows)
        return .windows
        #elseif os(Linux)
        return .linux
        #else
        return .unknown
        #endif
    }
}
```

### 2. `CPUFeatureSet` (硬件特性与指令集能力掩码)

```swift
public struct CPUFeatureSet: Sendable, Codable, Equatable {
    public let architecture: String
    public let logicalCores: Int
    public let physicalPageSize: Int
    public let hasARMNeon: Bool
    public let hasARMCrypto: Bool
    public let hasAESNI: Bool
    public let hasAVX2: Bool
    public let hasAVX512: Bool
    public let hasHardwareCRC32: Bool
    
    public init(
        architecture: String,
        logicalCores: Int,
        physicalPageSize: Int,
        hasARMNeon: Bool,
        hasARMCrypto: Bool,
        hasAESNI: Bool,
        hasAVX2: Bool,
        hasAVX512: Bool,
        hasHardwareCRC32: Bool
    ) {
        self.architecture = architecture
        self.logicalCores = logicalCores
        self.physicalPageSize = physicalPageSize
        self.hasARMNeon = hasARMNeon
        self.hasARMCrypto = hasARMCrypto
        self.hasAESNI = hasAESNI
        self.hasAVX2 = hasAVX2
        self.hasAVX512 = hasAVX512
        self.hasHardwareCRC32 = hasHardwareCRC32
    }
}
```

### 3. `PlatformPathNormalizationResult` (跨平台路径清理与安全分析结果)

```swift
public struct PlatformPathNormalizationResult: Sendable, Equatable {
    public let originalPath: String
    public let normalizedPath: String
    public let isAbsolute: Bool
    public let isUNCPath: Bool
    public let isLongPath: Bool
    public let containsWindowsReservedDeviceName: Bool
    public let strippedAlternateDataStream: String?
    public let win32FormattedPath: String
    
    public init(
        originalPath: String,
        normalizedPath: String,
        isAbsolute: Bool,
        isUNCPath: Bool,
        isLongPath: Bool,
        containsWindowsReservedDeviceName: Bool,
        strippedAlternateDataStream: String? = nil,
        win32FormattedPath: String
    ) {
        self.originalPath = originalPath
        self.normalizedPath = normalizedPath
        self.isAbsolute = isAbsolute
        self.isUNCPath = isUNCPath
        self.isLongPath = isLongPath
        self.containsWindowsReservedDeviceName = containsWindowsReservedDeviceName
        self.strippedAlternateDataStream = strippedAlternateDataStream
        self.win32FormattedPath = win32FormattedPath
    }
}
```

### 4. `PlatformFileAttributes` (跨平台统一文件元数据)

```swift
public struct PlatformFileAttributes: Sendable, Equatable {
    public let size: Int64
    public let isDirectory: Bool
    public let isSymbolicLink: Bool
    public let creationTimeUnixSec: Int64
    public let modificationTimeUnixSec: Int64
    public let posixPermissions: UInt32
    public let isReadOnly: Bool
    public let isHidden: Bool
    
    public init(
        size: Int64,
        isDirectory: Bool,
        isSymbolicLink: Bool,
        creationTimeUnixSec: Int64,
        modificationTimeUnixSec: Int64,
        posixPermissions: UInt32,
        isReadOnly: Bool,
        isHidden: Bool
    ) {
        self.size = size
        self.isDirectory = isDirectory
        self.isSymbolicLink = isSymbolicLink
        self.creationTimeUnixSec = creationTimeUnixSec
        self.modificationTimeUnixSec = modificationTimeUnixSec
        self.posixPermissions = posixPermissions
        self.isReadOnly = isReadOnly
        self.isHidden = isHidden
    }
}
```

---

## 二、 字段与 JSON Schema 双向一致性

所有实体均在 `contracts/` 下提供 1:1 对应的 JSON Schema 契约文件，严禁任何 `Any`、`object` 通配类型。
