# Data Model: 072-cli-packaging-homebrew-gui-integration

## 1. Release Packaging & Homebrew Entities

```swift
/// Release 打包构建配置
public struct CLIPackageConfig: Sendable, Equatable {
    public let version: String
    public let targetArchitecture: TargetArch
    public let stripSymbols: Bool
    public let generateDsym: Bool
    public let outputDirectory: String
    public let homebrewFormulaPath: String
    
    public enum TargetArch: String, Sendable, CaseIterable {
        case universal = "universal"
        case arm64 = "arm64"
        case x86_64 = "x86_64"
    }
}

/// Release 打包产物清册与校验和
public struct CLIPackageManifest: Sendable, Codable, Equatable {
    public let version: String
    public let tarballName: String
    public let tarballPath: String
    public let tarballByteSize: Int64
    public let sha256Checksum: String
    public let machOArchitectures: [String]
    public let manPageIncluded: Bool
    public let completionsIncluded: [String]
    public let formulaPath: String
}
```

## 2. Desktop GUI Inspector & Diagnostic View Models

```swift
/// 归档标准检查与属性检视展示状态
public struct ArchiveInspectorState: Sendable, Equatable {
    public let filePath: String
    public let fileName: String
    public let fileByteSize: Int64
    public let detectedFormat: ArchiveCompressionFormat?
    public let standardSpec: ArchiveFormatStandardSpec?
    public let signatureMatches: [ArchiveMagicSignature]
    public let parsedExtraFields: ParsedZipExtraFields?
    public let complianceReport: StandardsComplianceReport?
    public let isScanning: Bool
    public let scanDurationMs: Double
    public let errorMessage: String?
}

/// 归档诊断快照缓存条目
public struct ArchiveDiagnosticsCacheKey: Hashable, Sendable {
    public let filePath: String
    public let fileByteSize: Int64
    public let modificationTimestamp: Double
}
```
