// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// Represents a hierarchical tree node for archive file and directory navigation.
public struct ArchiveTreeNode: Identifiable, Sendable, Equatable {
    public var id: String { path }
    public let name: String
    public let path: String
    public let uncompressedSize: Int64
    public let isDirectory: Bool
    public let detectedEncoding: String
    public var children: [ArchiveTreeNode]?
    public var entry: ArchiveEntry?
    
    public init(
        id: String,
        name: String,
        path: String,
        uncompressedSize: Int64,
        isDirectory: Bool,
        detectedEncoding: String = "UTF-8",
        children: [ArchiveTreeNode]? = nil,
        entry: ArchiveEntry? = nil
    ) {
        self.name = name
        self.path = path
        self.uncompressedSize = uncompressedSize
        self.isDirectory = isDirectory
        self.detectedEncoding = detectedEncoding
        self.children = children
        self.entry = entry
    }
}

// MARK: - PrototypeCopyable Prototype Pattern Extension
extension ArchiveTreeNode: PrototypeCopyable {
    /// Creates a deep clone of the entire tree hierarchy.
    public func clone() -> ArchiveTreeNode {
        return cloneTree()
    }
    
    /// Recursively deep-copies this tree node and all descendants.
    /// - Returns: Independent cloned `ArchiveTreeNode` subtree.
    public func cloneTree() -> ArchiveTreeNode {
        let clonedChildren = children?.map { $0.cloneTree() }
        return ArchiveTreeNode(
            id: self.id,
            name: self.name,
            path: self.path,
            uncompressedSize: self.uncompressedSize,
            isDirectory: self.isDirectory,
            detectedEncoding: self.detectedEncoding,
            children: clonedChildren,
            entry: self.entry
        )
    }
}

// MARK: - ArchiveComponentProtocol Composite Pattern Extension
extension ArchiveTreeNode: ArchiveComponentProtocol {
    public var sizeBytes: Int64 {
        if isDirectory, let children = children, !children.isEmpty {
            return children.reduce(0) { $0 + $1.sizeBytes }
        }
        return uncompressedSize
    }
    
    public func getChildren() -> [ArchiveComponentProtocol] {
        guard let children = children, !children.isEmpty else { return [] }
        var result = [ArchiveComponentProtocol]()
        result.reserveCapacity(children.count)
        for child in children {
            result.append(child)
        }
        return result
    }
    
    /// Converts this node into a composite Component (Leaf or Composite Directory).
    public func toComponent() -> ArchiveComponentProtocol {
        if isDirectory {
            let childComponents = (children ?? []).map { $0.toComponent() }
            return ArchiveCompositeDirectory(name: name, path: path, entry: entry, children: childComponents)
        } else {
            return ArchiveLeafFile(name: name, path: path, sizeBytes: uncompressedSize, entry: entry)
        }
    }
    
    /// Reconstructs an `ArchiveTreeNode` from a composite Component.
    public init(component: ArchiveComponentProtocol, detectedEncoding: String = "UTF-8") {
        self.name = component.name
        self.path = component.path
        self.isDirectory = component.isDirectory
        self.detectedEncoding = detectedEncoding
        
        if let leaf = component as? ArchiveLeafFile {
            self.uncompressedSize = leaf.sizeBytes
            self.entry = leaf.entry
            self.children = nil
        } else if let composite = component as? ArchiveCompositeDirectory {
            self.entry = composite.entry
            let childComponents = composite.getChildren()
            let childNodes = childComponents.map { ArchiveTreeNode(component: $0, detectedEncoding: detectedEncoding) }
            self.uncompressedSize = childNodes.reduce(0) { $0 + $1.uncompressedSize }
            self.children = childNodes
        } else {
            self.uncompressedSize = 0
            self.entry = nil
            self.children = nil
        }
    }
}

/// Builds a hierarchical list of `ArchiveTreeNode` objects from flat `ArchiveEntry` lists.
public final class ArchiveTreeBuilder: @unchecked Sendable {
    public static func buildTree(from entries: [ArchiveEntry]) -> [ArchiveTreeNode] {
        return FastArchiveTreeBuilder.buildTree(from: entries)
    }
}

// MARK: - Component Protocol

//
//


// MARK: - Composite Pattern Component Protocol

/// Composite Pattern Core Interface: Unifies leaf files and composite directory containers.
public protocol ArchiveComponentProtocol: Sendable {
    /// Item name (e.g. "document.txt" or "Photos").
    var name: String { get }
    
    /// Item relative or absolute filesystem path.
    var path: String { get }
    
    /// Whether this component represents a directory container.
    var isDirectory: Bool { get }
    
    /// Total aggregate uncompressed byte size of component and all nested children.
    var sizeBytes: Int64 { get }
    
    /// Obtains direct child components (empty array for leaf files).
    func getChildren() -> [ArchiveComponentProtocol]
}

// MARK: - Leaf Node: Single File

/// Represents a single file entry in the composite tree structure.
public final class ArchiveLeafFile: ArchiveComponentProtocol, Identifiable, Equatable, @unchecked Sendable {
    public var id: String { path }
    public let name: String
    public let path: String
    public let sizeBytes: Int64
    public let isDirectory: Bool = false
    public let entry: ArchiveEntry?
    public let modificationDate: Date?
    public let compressedSizeBytes: Int64?
    public let crc32: UInt32?
    
    public init(
        name: String,
        path: String,
        sizeBytes: Int64,
        entry: ArchiveEntry? = nil,
        modificationDate: Date? = nil,
        compressedSizeBytes: Int64? = nil,
        crc32: UInt32? = nil
    ) {
        self.name = name
        self.path = path
        self.sizeBytes = sizeBytes
        self.entry = entry
        self.modificationDate = modificationDate
        self.compressedSizeBytes = compressedSizeBytes
        self.crc32 = crc32
    }
    
    public func getChildren() -> [ArchiveComponentProtocol] {
        return []
    }
    
    public static func == (lhs: ArchiveLeafFile, rhs: ArchiveLeafFile) -> Bool {
        return lhs.path == rhs.path && lhs.sizeBytes == rhs.sizeBytes
    }
}

// MARK: - Composite Container Node: Directory

/// Represents an immutable directory container holding child files and subdirectories.
public struct ArchiveCompositeDirectory: ArchiveComponentProtocol, Identifiable, Equatable, Sendable {
    public var id: String { path }
    public let name: String
    public let path: String
    public var isDirectory: Bool { true }
    public let entry: ArchiveEntry?
    public let modificationDate: Date?
    
    private var subdirectories: [String: ArchiveCompositeDirectory] = [:]
    private var leafFiles: [String: ArchiveLeafFile] = [:]
    private var customComponents: [String: ArchiveComponentProtocol] = [:]
    
    public init(
        name: String,
        path: String,
        entry: ArchiveEntry? = nil,
        modificationDate: Date? = nil,
        children: [ArchiveComponentProtocol] = []
    ) {
        self.name = name
        self.path = path
        self.entry = entry
        self.modificationDate = modificationDate
        for child in children {
            addDirect(component: child)
        }
    }
    
    /// Aggregate byte size computed recursively across all children without existential unboxing.
    public var sizeBytes: Int64 {
        var total: Int64 = 0
        for dir in subdirectories.values { total += dir.sizeBytes }
        for file in leafFiles.values { total += file.sizeBytes }
        for custom in customComponents.values { total += custom.sizeBytes }
        return total
    }

    public func totalFileCount() -> Int {
        var count = leafFiles.count
        for dir in subdirectories.values { count += dir.totalFileCount() }
        for custom in customComponents.values { count += custom.flattenLeaves().count }
        return count
    }
    
    public func totalDirectoryCount() -> Int {
        var count = subdirectories.count
        for dir in subdirectories.values { count += dir.totalDirectoryCount() }
        for custom in customComponents.values where custom.isDirectory { count += 1 }
        return count
    }
    
    public func renderTree(indent: String = "") -> String {
        var result = "\(name)\n"
        let children = getChildren()
        for (i, child) in children.enumerated() {
            let isLast = (i == children.count - 1)
            let branch = isLast ? "└── " : "├── "
            let childIndent = indent + (isLast ? "    " : "│   ")
            if let dir = child as? ArchiveCompositeDirectory {
                result += indent + branch + dir.renderTree(indent: childIndent)
            } else {
                result += indent + branch + "\(child.name) (\(child.sizeBytes) B)\n"
            }
        }
        return result
    }
    
    /// Obtains unsorted child items in O(1) time (bypasses locale sorting for sampling).
    public func getChildrenUnsorted() -> [ArchiveComponentProtocol] {
        var list = [ArchiveComponentProtocol]()
        list.reserveCapacity(subdirectories.count + leafFiles.count + customComponents.count)
        for dir in subdirectories.values { list.append(dir) }
        for file in leafFiles.values { list.append(file) }
        for custom in customComponents.values { list.append(custom) }
        return list
    }
    
    /// Obtains child items sorted with directories first and alphabetical name order.
    public func getChildren() -> [ArchiveComponentProtocol] {
        var list = [ArchiveComponentProtocol]()
        list.reserveCapacity(subdirectories.count + leafFiles.count + customComponents.count)
        let sortedDirs = subdirectories.values.sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }.map { $0 as ArchiveComponentProtocol }
        let sortedFiles = leafFiles.values.sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }.map { $0 as ArchiveComponentProtocol }
        for dir in sortedDirs { list.append(dir) }
        for file in sortedFiles { list.append(file) }
        if !customComponents.isEmpty {
            let sortedCustom = customComponents.values.sorted { a, b in
                if a.isDirectory != b.isDirectory {
                    return a.isDirectory && !b.isDirectory
                }
                return a.name.localizedStandardCompare(b.name) == .orderedAscending
            }
            for custom in sortedCustom { list.append(custom) }
        }
        return list
    }
    
    /// Internal direct child insertion.
    public mutating func addDirect(component: ArchiveComponentProtocol) {
        if let dir = component as? ArchiveCompositeDirectory {
            subdirectories[dir.name] = dir
        } else if let file = component as? ArchiveLeafFile {
            leafFiles[file.name] = file
        } else {
            customComponents[component.name] = component
        }
    }

    /// Internal direct child lookup.
    public func findChildDirect(named name: String) -> ArchiveComponentProtocol? {
        if let dir = subdirectories[name] { return dir }
        if let file = leafFiles[name] { return file }
        return customComponents[name]
    }

    /// Adds a child component.
    public mutating func add(component: ArchiveComponentProtocol) {
        addDirect(component: component)
    }
    
    /// Removes a child component by name.
    public mutating func remove(componentNamed name: String) {
        subdirectories.removeValue(forKey: name)
        leafFiles.removeValue(forKey: name)
        customComponents.removeValue(forKey: name)
    }
    
    /// Clears all child components.
    public mutating func removeAll() {
        subdirectories.removeAll()
        leafFiles.removeAll()
        customComponents.removeAll()
    }
    
    /// Finds a direct child component by name.
    public func findChild(named name: String) -> ArchiveComponentProtocol? {
        return findChildDirect(named: name)
    }

    /// High-performance recursive leaf flattener avoiding existential container conversions.
    public func flattenLeavesFast() -> [ArchiveLeafFile] {
        var leaves = [ArchiveLeafFile]()
        leaves.reserveCapacity(leafFiles.count)
        leaves.append(contentsOf: leafFiles.values)
        for dir in subdirectories.values {
            leaves.append(contentsOf: dir.flattenLeavesFast())
        }
        for custom in customComponents.values {
            leaves.append(contentsOf: custom.flattenLeaves())
        }
        return leaves
    }
    
    public static func == (lhs: ArchiveCompositeDirectory, rhs: ArchiveCompositeDirectory) -> Bool {
        return lhs.path == rhs.path &&
            lhs.subdirectories.count == rhs.subdirectories.count &&
            lhs.leafFiles.count == rhs.leafFiles.count &&
            lhs.customComponents.count == rhs.customComponents.count
    }
}

/// Backward-compatible and semantic alias for immutable directory tree nodes.
public typealias ArchiveDirectoryNode = ArchiveCompositeDirectory

// MARK: - ArchiveComponentTreeBuilder

public enum ArchiveComponentTreeBuilder {
    private final class MutableDirNode {
        let name: String
        let path: String
        var entry: ArchiveEntry?
        var modificationDate: Date?
        var subdirectories: [String: MutableDirNode] = [:]
        var leafFiles: [String: ArchiveLeafFile] = [:]
        var customChildren: [String: ArchiveComponentProtocol] = [:]
        
        init(name: String, path: String, entry: ArchiveEntry? = nil, modificationDate: Date? = nil) {
            self.name = name
            self.path = path
            self.entry = entry
            self.modificationDate = modificationDate
        }
        
        func toImmutableDirectory() -> ArchiveCompositeDirectory {
            var dir = ArchiveCompositeDirectory(
                name: name,
                path: path,
                entry: entry,
                modificationDate: modificationDate
            )
            for (_, subDir) in subdirectories {
                dir.addDirect(component: subDir.toImmutableDirectory())
            }
            for (_, leaf) in leafFiles {
                dir.addDirect(component: leaf)
            }
            for (_, custom) in customChildren {
                dir.addDirect(component: custom)
            }
            return dir
        }
    }

    public static func buildTree(from entries: [ArchiveEntry]) -> ArchiveCompositeDirectory {
        let root = MutableDirNode(name: "root", path: "")
        for entry in entries {
            let parts = entry.path.split(separator: "/").map(String.init)
            guard !parts.isEmpty else { continue }
            
            var current = root
            for i in 0..<(parts.count - 1) {
                let dirName = parts[i]
                let dirPath = parts[0...i].joined(separator: "/")
                if let existing = current.subdirectories[dirName] {
                    current = existing
                } else {
                    let newDir = MutableDirNode(name: dirName, path: dirPath)
                    current.subdirectories[dirName] = newDir
                    current = newDir
                }
            }
            
            let leafName = parts.last!
            if entry.isDirectory {
                if current.subdirectories[leafName] == nil {
                    let newDir = MutableDirNode(name: leafName, path: entry.path, entry: entry, modificationDate: entry.modificationDate)
                    current.subdirectories[leafName] = newDir
                }
            } else {
                let leaf = ArchiveLeafFile(
                    name: leafName,
                    path: entry.path,
                    sizeBytes: entry.uncompressedSize,
                    entry: entry,
                    modificationDate: entry.modificationDate,
                    compressedSizeBytes: nil,
                    crc32: nil
                )
                current.leafFiles[leafName] = leaf
            }
        }
        return root.toImmutableDirectory()
    }

    public static func buildTree(fromDiskPath diskPath: String) -> ArchiveCompositeDirectory {
        let url = URL(fileURLWithPath: diskPath)
        let name = url.lastPathComponent
        let root = MutableDirNode(name: name, path: diskPath)
        
        let fm = FileManager.default
        guard let enumerator = fm.enumerator(at: url, includingPropertiesForKeys: [.fileSizeKey, .isDirectoryKey]) else {
            return root.toImmutableDirectory()
        }
        
        for case let fileURL as URL in enumerator {
            let relativePath = fileURL.path.replacingOccurrences(of: diskPath + "/", with: "")
            let resourceValues = try? fileURL.resourceValues(forKeys: [.fileSizeKey, .isDirectoryKey])
            let isDir = resourceValues?.isDirectory ?? false
            let size = Int64(resourceValues?.fileSize ?? 0)
            
            let parts = relativePath.split(separator: "/").map(String.init)
            guard !parts.isEmpty else { continue }
            
            var current = root
            for i in 0..<(parts.count - 1) {
                let dirName = parts[i]
                let dirPath = diskPath + "/" + parts[0...i].joined(separator: "/")
                if let existing = current.subdirectories[dirName] {
                    current = existing
                } else {
                    let newDir = MutableDirNode(name: dirName, path: dirPath)
                    current.subdirectories[dirName] = newDir
                    current = newDir
                }
            }
            
            let leafName = parts.last!
            if isDir {
                if current.subdirectories[leafName] == nil {
                    let newDir = MutableDirNode(name: leafName, path: fileURL.path)
                    current.subdirectories[leafName] = newDir
                }
            } else {
                let leaf = ArchiveLeafFile(name: leafName, path: fileURL.path, sizeBytes: size)
                current.leafFiles[leafName] = leaf
            }
        }
        return root.toImmutableDirectory()
    }
}

extension ArchiveComponentProtocol {
    public func flattenLeaves() -> [ArchiveLeafFile] {
        if let leaf = self as? ArchiveLeafFile {
            return [leaf]
        }
        if let dir = self as? ArchiveCompositeDirectory {
            return dir.flattenLeavesFast()
        }
        return getChildren().flatMap { $0.flattenLeaves() }
    }
}

// MARK: - Archive Filter

//
//


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

//
//


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
