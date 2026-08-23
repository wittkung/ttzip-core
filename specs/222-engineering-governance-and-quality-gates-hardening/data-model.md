# Data Model & Schema: Quality Gates & Governance

## 1. C-ABI Symbol Manifest Schema

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "CAbiSymbolManifest",
  "type": "object",
  "required": ["version", "header_file", "library_file", "total_symbols", "symbols"],
  "properties": {
    "version": { "type": "string" },
    "header_file": { "type": "string" },
    "library_file": { "type": "string" },
    "total_symbols": { "type": "integer" },
    "symbols": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["name", "return_type", "status"],
        "properties": {
          "name": { "type": "string" },
          "return_type": { "type": "string" },
          "status": { "type": "string", "enum": ["present", "missing", "deprecated"] }
        }
      }
    }
  }
}
```

## 2. Differential Transaction Journal Model

```swift
public struct DifferentialExtractJournal: Sendable {
    public struct JournalEntry: Sendable {
        public let path: String
        public let isDirectory: Bool
        public let timestamp: Date
    }

    public private(set) var createdEntries: [JournalEntry]
    public let destinationDirectory: String

    public mutating func recordCreated(path: String, isDirectory: Bool) {
        createdEntries.append(JournalEntry(path: path, isDirectory: isDirectory, timestamp: Date()))
    }

    public func executeRollback(fileManager: FileManager = .default) {
        // Reverse order deletion: files first, then directories
        for entry in createdEntries.reversed() {
            try? fileManager.removeItem(atPath: entry.path)
        }
    }
}
```
