// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// Model representing a recently accessed archive file record.
public struct RecentArchiveRecord: Identifiable, Codable, Equatable, Hashable, Sendable {
    public var id: String { path }
    public let path: String
    public let name: String
    public let extensionName: String
    public let date: Date
    
    public init(path: String, date: Date = Date()) {
        self.path = path
        self.name = (path as NSString).lastPathComponent
        self.extensionName = (path as NSString).pathExtension.uppercased()
        self.date = date
    }
}
