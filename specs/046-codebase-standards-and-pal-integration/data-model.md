# Data Model: 046-codebase-standards-and-pal-integration

本数据模型定义核心业务模块与 PAL 交互的统一契约：

---

## 一、 实体定义 (Entities)

### 1. `PathSanitizationSummary` (路径安全综合审计模型)

```swift
public struct PathSanitizationSummary: Sendable, Codable, Equatable {
    public let originalPath: String
    public let cleanRelativePath: String
    public let isPathSafe: Bool
    public let hasWindowsReservedName: Bool
    public let hasAlternateDataStream: Bool
    public let isLongPath: Bool
}
```

---

## 二、 契约文件索引

- `contracts/path_sanitization_summary_schema.json`: 强类型 Schema 定义。
