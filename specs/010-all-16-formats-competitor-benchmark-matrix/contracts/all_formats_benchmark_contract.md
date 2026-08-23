# Interface Contract: 16-Format Competitor Benchmark

## Swift API Contract
```swift
extension CompetitorBenchmarkRunner {
    public static func runCompetitorMatrix(
        selectedFormats: [ArchiveCompressionFormat]? = nil,
        selectedLevels: [ArchiveCompressionLevel]? = nil,
        selectedTools: [String]? = nil,
        hugeSizeBytes: Int64 = 500 * 1024 * 1024,
        customFilePaths: [String]? = nil,
        stopOnLagOrError: Bool = false,
        autoBestCompetitor: Bool = true,
        passes: Int = 1,
        progressHandler: (@Sendable (String) -> Void)? = nil
    ) async throws -> [CompetitorBenchmarkRow]
}
```
