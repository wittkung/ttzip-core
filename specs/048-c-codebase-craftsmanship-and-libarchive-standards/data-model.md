# Data Model: 048-c-codebase-craftsmanship-and-libarchive-standards

本数据模型定义 C 桥接层的强类型内存所有权与确界约束规范。

---

## 一、 实体定义 (Entities)

### 1. `CBridgeContractAudit` (C 桥接层契约审计模型)

```swift
public struct CBridgeContractAudit: Sendable, Codable, Equatable {
    public let headerFile: String
    public let sourceFile: String
    public let hasDoxygenDocBlock: Bool
    public let hasOwnershipAnnotation: Bool
    public let hasClampingProtection: Bool
    public let hasSymmetricDeallocation: Bool
}
```

---

## 二、 契约文件索引

- `contracts/c_bridge_contract_schema.json`: 强类型 Schema 定义。
