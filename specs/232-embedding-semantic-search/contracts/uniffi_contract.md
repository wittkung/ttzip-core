# UniFFI & Swift Interface Contract v1: Hybrid Semantic Search (Feature 232)

**Location**: `products/ttzip/core/specs/232-embedding-semantic-search/contracts/uniffi_contract.md`  
**Updated**: 2026-10-18  
**Status**: Frozen v1 Contract (Ready for **W1** UI & **H** Build Integration)  

---

## 1. Interface v1 Design Guarantees

1. **Model Status, Install Progress & Error Reporting**: Explicit state machine (`notInstalled`, `downloading`, `verifying`, `ready`, `failed`) with offline lexical fallback (`degradedLexicalOnly = true`).
2. **Authorized Scope Identity**: Every directory or open archive is registered via `SemanticScopeDescriptor` (`scopeId`, `rootPath`, `kind`); revoking a scope immediately cancels indexing, deletes its persisted index, and filters out its hits.
3. **Indexing Lifecycle**: Start/cancel indexing with real-time `SemanticIndexProgress` (`scanning`, `tier1Paths`, `tier2Content`, `completed`, `cancelled`, `failed`) and incremental file events (`created`, `modified`, `renamed`, `deleted`).
4. **Request-ID & Window-ID Query Isolation**: Every query carries `requestId: UInt64` and `windowId: String`. Cooperative cancellation covers both queued and in-flight work; stale `requestId` responses are rejected before reaching UI state.
5. **Rich Hit Metadata & Archive `entrySubpath` Preservation**: Every `SemanticSearchHit` includes `sourceKind`, `containerPath`, `relativePath` (the archive inner `entrySubpath` for archive hits), `snippet`, `lineStart`, `lineEnd`, `matchKind` (`lexicalExact`, `pathSemantic`, `contentSemantic`, `hybrid`), `indexVersion`, and `modelFingerprint`. Calling `buildOpenRequest(for:)` returns a `SemanticOpenRequest` that preserves `entrySubpath` and `lineStart`.

---

## 2. Swift Interface v1 (`SemanticSearchContracts.swift` & `SemanticSearchEngine.swift`)

```swift
public enum SemanticComputeBackend: String, Sendable, Equatable, Codable {
    case metal
    case cpuFallback
    case deterministicTestDouble
}

public struct SemanticModelInfo: Sendable, Equatable, Codable {
    public let modelId: String              // "google/embeddinggemma-2"
    public let revision: String             // "914f7f8"
    public let ggufFilename: String         // "embeddinggemma-2-Q8_0.gguf"
    public let expectedBytes: Int64         // 309_855_456
    public let expectedSha256: String       // "2188ac1deca4b77dffefd603c2776a9d76d9d74ec01841392982ebb840b09135"
    public let modelFingerprint: UInt64     // 0x914F_7F80_B114_6101
    public let backend: SemanticComputeBackend
}

public enum SemanticModelError: Error, Sendable, Equatable, LocalizedError {
    case modelFileMissing(expectedPath: String)
    case sizeMismatch(expected: Int64, actual: Int64)
    case checksumMismatch(expected: String, actual: String)
    case downloadFailed(reason: String)
    case runtimeLoadFailed(reason: String)
}

public enum SemanticModelState: Sendable, Equatable {
    case notInstalled
    case downloading(bytesDownloaded: Int64, totalBytes: Int64)
    case verifying
    case ready(info: SemanticModelInfo)
    case failed(error: SemanticModelError)
}

public enum SemanticScopeKind: String, Sendable, Equatable, Codable {
    case directory
    case openArchive
}

public struct SemanticScopeDescriptor: Sendable, Equatable, Codable, Identifiable {
    public var id: String { scopeId }
    public let scopeId: String
    public let rootPath: String
    public let displayName: String
    public let kind: SemanticScopeKind
    public let isAuthorized: Bool
}

public enum SemanticMatchKind: String, Sendable, Equatable, Codable {
    case lexicalExact
    case pathSemantic
    case contentSemantic
    case hybrid
}

public struct SemanticSearchHit: Sendable, Equatable, Codable, Identifiable {
    public var id: String { hitId }
    public let hitId: String
    public let scopeId: String
    public let sourceKind: SemanticScopeKind
    public let containerPath: String
    public let relativePath: String         // Inner entrySubpath when sourceKind == .openArchive
    public let matchKind: SemanticMatchKind
    public let snippet: String?
    public let lineStart: Int?
    public let lineEnd: Int?
    public let lexicalScore: Float
    public let semanticCosine: Float?
    public let fusedScore: Float
    public let indexVersion: UInt32
    public let modelFingerprint: UInt64
}

public struct SemanticOpenRequest: Sendable, Equatable {
    public let scopeId: String
    public let sourceKind: SemanticScopeKind
    public let containerURL: URL
    public let entrySubpath: String?        // Non-nil for archive inner entries!
    public let focusLine: Int?
}
```

---

## 3. Compilable W1 Calling Example

```swift
let engine = SemanticSearchEngine.shared

// 1. Register user-authorized folder or mounted archive
let scope = await engine.registerAuthorizedDirectory(url: downloadsURL)
_ = await engine.startIndexing(scopeId: scope.scopeId) { progress in
    // Update indexing progress pill in UI
}

// 2. Execute request_id-isolated search from GlobalSearchPanelController
let request = SemanticSearchRequest(
    requestId: nextRequestId,
    windowId: "global-search-panel",
    query: userQuery,
    scopeIds: [scope.scopeId],
    limit: 50
)
let response = try await engine.search(request)

// 3. Open hit while preserving inner archive entrySubpath
if let selectedHit = response.hits.first {
    let openReq = engine.buildOpenRequest(for: selectedHit)
    // openReq.containerURL -> outer .zip or folder
    // openReq.entrySubpath -> inner archive entry path (never discarded!)
}
```
