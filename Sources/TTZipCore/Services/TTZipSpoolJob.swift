// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import Foundation

/// Canonical structured job payload exchanged between Finder extension and TTZipApp via App Group spooling.
public struct TTZipSpoolJob: Codable, Sendable, Equatable {
    public let jobId: UUID
    public let createdAt: Date
    public let action: String
    public let targetFormat: String?
    public let sourcePaths: [String]
    public let sourceBookmarks: [Data]
    public let destinationDirectoryBookmark: Data?
    public let compressionLevel: Int?
    public let password: String?
    
    public init(
        jobId: UUID = UUID(),
        createdAt: Date = Date(),
        action: String,
        targetFormat: String? = nil,
        sourcePaths: [String] = [],
        sourceBookmarks: [Data] = [],
        destinationDirectoryBookmark: Data? = nil,
        compressionLevel: Int? = nil,
        password: String? = nil
    ) {
        self.jobId = jobId
        self.createdAt = createdAt
        self.action = action
        self.targetFormat = targetFormat
        self.sourcePaths = sourcePaths
        self.sourceBookmarks = sourceBookmarks
        self.destinationDirectoryBookmark = destinationDirectoryBookmark
        self.compressionLevel = compressionLevel
        self.password = password
    }
    
    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        jobId = try container.decode(UUID.self, forKey: .jobId)
        createdAt = try container.decode(Date.self, forKey: .createdAt)
        action = try container.decode(String.self, forKey: .action)
        targetFormat = try container.decodeIfPresent(String.self, forKey: .targetFormat)
        sourcePaths = try container.decodeIfPresent([String].self, forKey: .sourcePaths) ?? []
        sourceBookmarks = try container.decodeIfPresent([Data].self, forKey: .sourceBookmarks) ?? []
        destinationDirectoryBookmark = try container.decodeIfPresent(Data.self, forKey: .destinationDirectoryBookmark)
        compressionLevel = try container.decodeIfPresent(Int.self, forKey: .compressionLevel)
        password = try container.decodeIfPresent(String.self, forKey: .password)
    }
}
