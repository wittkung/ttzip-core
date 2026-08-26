# Data Model: 047-libarchive-elegance-and-decoupling

本数据模型定义容器格式、流式滤镜正交正交解耦与状态机体系契约。

---

## 一、 实体定义 (Entities)

### 1. `ArchiveContainerFormat` (归档容器格式)

```swift
/// 负责归档目录结构、条目标头与元数据编解码的独立容器格式
public enum ArchiveContainerFormat: String, Sendable, CaseIterable, Codable {
    case zip
    case sevenZip = "7z"
    case tar
    case cpio
    case ar
    case iso
    case wim
    case raw
}
```

### 2. `ArchiveStreamFilter` (流式压缩滤镜)

```swift
/// 负责纯字节流压缩/解压与变换的正交滤镜
public enum ArchiveStreamFilter: String, Sendable, CaseIterable, Codable {
    case none
    case gzip
    case bzip2
    case xz
    case zstd
    case lz4
    case brotli
    case lzip
    case lrzip
}
```

### 3. `ArchivePipelineComposition` (正交管道组合配置)

```swift
/// 描述容器格式与流式滤镜的组合模型
public struct ArchivePipelineComposition: Sendable, Codable, Equatable {
    public let container: ArchiveContainerFormat
    public let filter: ArchiveStreamFilter
    public let supportsFastPathBypass: Bool
    public let displayName: String
    public let primaryFileExtension: String
}
```

### 4. `TTZipStatus` (统一 6 级错误与状态码)

```swift
/// 对标 libarchive 状态码体系
public enum TTZipStatus: Int32, Sendable, Codable {
    case eof = 1
    case ok = 0
    case retry = -10
    case warn = -20
    case failed = -25
    case fatal = -30
}
```

---

## 二、 契约文件索引

- `contracts/pipeline_composition_schema.json`: 强类型 Schema 定义。
- `contracts/engine_status_schema.json`: 状态码 Schema 定义。
