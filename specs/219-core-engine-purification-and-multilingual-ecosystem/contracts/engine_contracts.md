# Contracts: Core Engine & Multi-Language Interfaces

**Feature**: `219-core-engine-purification-and-multilingual-ecosystem`  

---

## 1. Rust Pure Engine Contract (`ttzip-engine`)

```rust
pub struct ArchiveEngine;

impl ArchiveEngine {
    pub fn create_archive(options: &CreateOptions) -> Result<CreateReport, EngineError>;
    pub fn extract_archive(options: &ExtractOptions) -> Result<ExtractReport, EngineError>;
    pub fn inspect_archive(path: &Path) -> Result<Vec<EntryMetadata>, EngineError>;
    pub fn compress_buffer(data: &[u8], codec: CodecType, level: i32) -> Result<Vec<u8>, EngineError>;
    pub fn decompress_buffer(data: &[u8], codec: CodecType) -> Result<Vec<u8>, EngineError>;
}
```

---

## 2. Node.js N-API Contract (`ttzip` npm)

```typescript
export interface EntryMetadata {
  path: string;
  uncompressedSize: number;
  compressedSize: number;
  crc32: number;
  isDirectory: boolean;
}

export function compress(inputs: string[], destination: string, options?: CompressOptions): Promise<void>;
export function extract(archivePath: string, destination: string, options?: ExtractOptions): Promise<void>;
export function inspect(archivePath: string): Promise<EntryMetadata[]>;
export function compressBuffer(data: Buffer, format?: string, level?: number): Buffer;
export function decompressBuffer(data: Buffer, format?: string): Buffer;
export function crc32(data: Buffer): number;
```
