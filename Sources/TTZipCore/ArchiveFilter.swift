// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// Compiled Filter evaluator and facade.
public final class ArchiveFilter: Sendable {
    public let expression: String
    
    /// Initializes a Filter expression.
    public init(expression: String) {
        self.expression = expression
    }
    
    /// Evaluates whether an archive entry satisfies the filter expression.
    public func evaluate(entry: ArchiveEntry) -> Bool {
        let trimmed = expression.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty { return true }
        
        if trimmed.contains(" OR ") {
            let parts = trimmed.components(separatedBy: " OR ")
            return parts.contains { ArchiveFilter(expression: $0).evaluate(entry: entry) }
        }
        if trimmed.contains(" AND ") {
            let parts = trimmed.components(separatedBy: " AND ")
            return parts.allSatisfy { ArchiveFilter(expression: $0).evaluate(entry: entry) }
        }
        
        if trimmed.hasPrefix("ext:") {
            let targetExt = String(trimmed.dropFirst(4)).trimmingCharacters(in: .whitespaces).lowercased()
            return entry.path.lowercased().hasSuffix("." + targetExt)
        }
        if trimmed.hasPrefix("size:>") {
            let valStr = String(trimmed.dropFirst(6)).trimmingCharacters(in: .whitespaces)
            let val = Int64(valStr) ?? 0
            return entry.uncompressedSize > val
        }
        if trimmed.hasPrefix("size:<") {
            let valStr = String(trimmed.dropFirst(6)).trimmingCharacters(in: .whitespaces)
            let val = Int64(valStr) ?? 0
            return entry.uncompressedSize < val
        }
        return entry.path.localizedCaseInsensitiveContains(trimmed)
    }
    
    /// One-shot static evaluation of an entry against an expression.
    public static func evaluate(expression: String, entry: ArchiveEntry) -> Bool {
        let filter = ArchiveFilter(expression: expression)
        return filter.evaluate(entry: entry)
    }
    
    /// Static evaluation of an entry against a query string.
    public static func evaluate(entry: ArchiveEntry, query: String) -> Bool {
        return evaluate(expression: query, entry: entry)
    }
    
    /// Filters a collection of archive entries using an expression.
    public static func filter(entries: [ArchiveEntry], expression: String) -> [ArchiveEntry] {
        let trimmed = expression.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty { return entries }
        let filter = ArchiveFilter(expression: trimmed)
        return entries.filter { filter.evaluate(entry: $0) }
    }
}

// MARK: - ArchiveFilterOptions DSL Extension

extension ArchiveFilterOptions {
    /// Evaluates whether an archive entry passes options and optional DSL query constraints.
    public func matches(entry: ArchiveEntry, dslQuery: String? = nil) -> Bool {
        if skipMacJunk {
            if entry.name == ".DS_Store" || entry.name.hasPrefix("._") || entry.path.contains("__MACOSX/") {
                return false
            }
        }
        if skipGitDirectory {
            if entry.name == ".git" || entry.path.contains("/.git/") || entry.path.hasPrefix(".git/") {
                return false
            }
        }
        for pattern in customIgnorePatterns {
            if entry.path.contains(pattern) || entry.name == pattern {
                return false
            }
        }
        
        if let query = dslQuery, !query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return ArchiveFilter.evaluate(expression: query, entry: entry)
        }
        
        return true
    }
}

// MARK: - Filter Options

/// Filter and entry exclusion rules for archive creation and extraction.
public struct ArchiveFilterOptions: Sendable, Equatable {
    /// Glob patterns to exclude.
    public var excludePatterns: [String]
    /// Glob patterns to include exclusively.
    public var includePatterns: [String]
    /// Number of leading path directory components to strip on extraction.
    public var stripComponents: Int
    /// Automatically ignore VCS repository metadata (`.git`, `.svn`, `.hg`).
    public var excludeVCS: Bool
    /// Automatically exclude AppleDouble (`._*`) and `.DS_Store` metadata artifacts.
    public var noMacMetadata: Bool
    /// Extract files directly into destination root without creating directories.
    public var flattenPaths: Bool
    /// Path to file containing newline or NUL-separated path list.
    public var filesFromPath: String?
    /// Whether `--files-from` list uses NUL delimiter (`\0`).
    public var nullDelimiter: Bool
    
    // MARK: - Backwards Compatibility Aliases
    public var skipMacJunk: Bool {
        get { noMacMetadata }
        set { noMacMetadata = newValue }
    }
    
    public var skipGitDirectory: Bool {
        get { excludeVCS }
        set { excludeVCS = newValue }
    }
    
    public var customIgnorePatterns: [String] {
        get { excludePatterns }
        set { excludePatterns = newValue }
    }
    
    public init(
        excludePatterns: [String] = [],
        includePatterns: [String] = [],
        stripComponents: Int = 0,
        excludeVCS: Bool = false,
        noMacMetadata: Bool = true,
        flattenPaths: Bool = false,
        filesFromPath: String? = nil,
        nullDelimiter: Bool = false
    ) {
        self.excludePatterns = excludePatterns
        self.includePatterns = includePatterns
        self.stripComponents = stripComponents
        self.excludeVCS = excludeVCS
        self.noMacMetadata = noMacMetadata
        self.flattenPaths = flattenPaths
        self.filesFromPath = filesFromPath
        self.nullDelimiter = nullDelimiter
    }
    
    public init(
        skipMacJunk: Bool = true,
        skipGitDirectory: Bool = false,
        customIgnorePatterns: [String] = []
    ) {
        self.excludePatterns = customIgnorePatterns
        self.includePatterns = []
        self.stripComponents = 0
        self.excludeVCS = skipGitDirectory
        self.noMacMetadata = skipMacJunk
        self.flattenPaths = false
        self.filesFromPath = nil
        self.nullDelimiter = false
    }
    
    public static let defaultClean = ArchiveFilterOptions(excludePatterns: [], includePatterns: [], stripComponents: 0, excludeVCS: false, noMacMetadata: true)
    public static let preserveAll = ArchiveFilterOptions(excludePatterns: [], includePatterns: [], stripComponents: 0, excludeVCS: false, noMacMetadata: false)
    
    /// Returns true if the entry path represents macOS, Windows, or POSIX PaxHeader system metadata artifacts.
    public static func isSystemMetadata(path: String) -> Bool {
        var normalized = path.replacingOccurrences(of: "\\", with: "/")
        while normalized.hasPrefix("./") {
            normalized.removeFirst(2)
        }
        let segments = normalized.split(separator: "/", omittingEmptySubsequences: true)
        if segments.isEmpty {
            return false
        }
        
        for seg in segments {
            let s = String(seg)
            if s == "__MACOSX" {
                return true
            }
            if s.hasPrefix("._") {
                return true
            }
            if s == ".DS_Store" || s == ".localized" || s == ".VolumeIcon.icns" {
                return true
            }
            if s.hasPrefix(".Spotlight-V100") || s.hasPrefix(".Trashes") || s.hasPrefix(".fseventsd") || s.hasPrefix(".TemporaryItems") {
                return true
            }
            if s == "PaxHeader" || s.hasPrefix("PaxHeaders.") || s.hasPrefix("PaxHeader.") {
                return true
            }
            if s.caseInsensitiveCompare("Thumbs.db") == .orderedSame ||
               s.caseInsensitiveCompare("desktop.ini") == .orderedSame ||
               s.caseInsensitiveCompare("ehthumbs.db") == .orderedSame ||
               s.caseInsensitiveCompare("$RECYCLE.BIN") == .orderedSame {
                return true
            }
        }
        return false
    }
}

// MARK: - PrototypeCopyable Prototype Pattern Extension
extension ArchiveFilterOptions: PrototypeCopyable {
    /// Creates an independent snapshot clone of this configuration.
    public func clone() -> ArchiveFilterOptions {
        return clone(mutate: { _ in })
    }
    
    /// Prototype copy with inout mutation closure.
    /// - Parameter mutate: Mutation block applied to the clone.
    /// - Returns: Mutated independent snapshot.
    public func clone(mutate: (inout ArchiveFilterOptions) -> Void = { _ in }) -> ArchiveFilterOptions {
        var copy = self
        mutate(&copy)
        return copy
    }
}
