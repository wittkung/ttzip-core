// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

// MARK: - ArchiveComponentProtocol Composite Pattern Core

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

extension ArchiveComponentProtocol {
    /// Flattens all leaf file descendants into a linear array.
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

// MARK: - Leaf Node: Single File

/// Represents an immutable single file entry in the composite tree structure with value semantics.
public struct ArchiveLeafFile: ArchiveComponentProtocol, Identifiable, Equatable, Sendable {
    public var id: String { path }
    public let name: String
    public let path: String
    public let sizeBytes: Int64
    public var isDirectory: Bool { false }
    public let entry: ArchiveEntry?
    public let modificationDate: Date?
    public let compressedSizeBytes: Int64?
    public let crc32: UInt32?
    public let posixPermissions: UInt32?
    public let isEncrypted: Bool?
    public let detectedEncoding: String?

    public init(
        name: String,
        path: String,
        sizeBytes: Int64,
        entry: ArchiveEntry? = nil,
        modificationDate: Date? = nil,
        compressedSizeBytes: Int64? = nil,
        crc32: UInt32? = nil,
        posixPermissions: UInt32? = nil,
        isEncrypted: Bool? = nil,
        detectedEncoding: String? = nil
    ) {
        self.name = name
        self.path = path
        self.sizeBytes = sizeBytes
        self.entry = entry
        self.modificationDate = modificationDate ?? entry?.modificationDate
        self.compressedSizeBytes = compressedSizeBytes ?? entry?.compressedSize
        self.crc32 = crc32 ?? entry?.crc32
        self.posixPermissions = posixPermissions ?? entry?.posixPermissions
        self.isEncrypted = isEncrypted ?? entry?.isEncrypted
        self.detectedEncoding = detectedEncoding ?? entry?.detectedEncoding
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
    public let compressedSizeBytes: Int64?
    public let crc32: UInt32?
    public let posixPermissions: UInt32?
    public let isEncrypted: Bool?
    public let detectedEncoding: String?

    private var subdirectories: [String: ArchiveCompositeDirectory] = [:]
    private var leafFiles: [String: ArchiveLeafFile] = [:]
    private var customComponents: [String: ArchiveComponentProtocol] = [:]

    public init(
        name: String,
        path: String,
        entry: ArchiveEntry? = nil,
        modificationDate: Date? = nil,
        compressedSizeBytes: Int64? = nil,
        crc32: UInt32? = nil,
        posixPermissions: UInt32? = nil,
        isEncrypted: Bool? = nil,
        detectedEncoding: String? = nil,
        children: [ArchiveComponentProtocol] = []
    ) {
        self.name = name
        self.path = path
        self.entry = entry
        self.modificationDate = modificationDate ?? entry?.modificationDate
        self.compressedSizeBytes = compressedSizeBytes ?? entry?.compressedSize
        self.crc32 = crc32 ?? entry?.crc32
        self.posixPermissions = posixPermissions ?? entry?.posixPermissions
        self.isEncrypted = isEncrypted ?? entry?.isEncrypted
        self.detectedEncoding = detectedEncoding ?? entry?.detectedEncoding
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
