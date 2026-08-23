# Data Model: Dedicated Per-Format Benchmark Charts

## 1. 专属格式专场定义 (Dedicated Benchmark Session)

```swift
public enum DedicatedFormatSession: String, Codable, CaseIterable, Identifiable, Sendable {
    case zip    = "zip"
    case sevenZ = "7z"
    case tarZst = "tar_zst"
    case lz4    = "lz4"
    case full   = "full_composite"

    public var id: String { rawValue }

    public var chartTitle: String {
        switch self {
        case .zip:    return "ZIP Format Pareto Benchmark (TTZip vs. 7-Zip vs. Apple Native)"
        case .sevenZ: return "7Z Format Pareto Benchmark (TTZip vs. 7-Zip Official ARM64)"
        case .tarZst: return "TAR.ZST Modern Stream Pareto Benchmark (TTZip Direct Pipeline)"
        case .lz4:    return "LZ4 Memory-Speed Pareto Benchmark (TTZip vs. System Native)"
        case .full:   return "macOS Compression Pareto Benchmark (4-Tier Multi-Software Suite)"
        }
    }

    public var pngFileName: String {
        switch self {
        case .zip:    return "pareto_pk_zip.png"
        case .sevenZ: return "pareto_pk_7z.png"
        case .tarZst: return "pareto_pk_tar_zst.png"
        case .lz4:    return "pareto_pk_lz4.png"
        case .full:   return "software_pareto_pk.png"
        }
    }

    public var svgFileName: String {
        switch self {
        case .zip:    return "pareto_pk_zip.svg"
        case .sevenZ: return "pareto_pk_7z.svg"
        case .tarZst: return "pareto_pk_tar_zst.svg"
        case .lz4:    return "pareto_pk_lz4.svg"
        case .full:   return "software_pareto_pk.svg"
        }
    }
}
```
