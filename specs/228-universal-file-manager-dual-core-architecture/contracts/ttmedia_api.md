# Contract: Universal Media & Rich Document Kit (`infra/ttmedia`)

**Module Target**: `//infra/ttmedia:ttmedia`  
**Language Compatibility**: Swift, C/C++  

---

## 1. Unified Inspection Protocol

```swift
public protocol MediaInspecting: Sendable {
    func inspect(url: URL) async throws -> MediaInspection
    func inspect(data: Data, suggestedFileName: String) async throws -> MediaInspection
}

public protocol MediaOffscreenRendering: Sendable {
    func renderThumbnail(url: URL, targetSize: CGSize) async throws -> CGImage
    func rasterizeVectorPage(url: URL, pageIndex: Int, scale: CGFloat) async throws -> CGImage
}
```

---

## 2. In-Place Edit Mutation Protocol

```swift
public protocol InPlaceEditableDocument: Sendable {
    func generateByteDelta(original: URL, modifiedContent: String) throws -> InPlaceDelta
}

public struct InPlaceDelta: Sendable {
    public let targetEntryPath: String
    public let offset: UInt64
    public let replacementBytes: Data
    public let originalLength: UInt64
    public let newLength: UInt64
}
```

---

## 3. Sub-Engine Decoupling Contracts

- `infra/ttmedia/av`: Depends directly on `//infra/ttmpv`. Provides audio waveform extraction from PCM samples and frame stepping.
- `infra/ttmedia/document`: Standalone Swift/Rust library parsing XLSX/CSV tables into flat memory buffers and DOCX/PDF text streams. Zero dependency on WebKit.
- `infra/ttmedia/syntax`: Standalone C/Rust library binding Tree-sitter core grammars.
- `infra/ttmedia/image`: Downsampled mipmap and tile pyramids with Metal pipeline shaders.
