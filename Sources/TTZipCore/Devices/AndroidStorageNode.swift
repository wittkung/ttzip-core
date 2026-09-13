// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// Type taxonomy of virtual file system nodes accessible on an Android storage partition.
public enum AndroidVfsEntryType: String, Sendable, Hashable, Codable, CaseIterable, CustomStringConvertible {
    /// Regular file with readable or writable payload stream.
    case file = "File"
    /// Standard navigable directory node.
    case directory = "Directory"
    /// Symbolic or hard filesystem link.
    case symlink = "Symlink"
    /// System-protected directory subject to Android Scoped Storage restrictions (e.g., /Android/data).
    case restrictedDirectory = "RestrictedDirectory"

    public var description: String {
        rawValue
    }
}

/// Represents a file or directory node in the remote virtual file system hierarchy.
public struct AndroidStorageNode: Identifiable, Sendable, Hashable, Codable {
    /// Full normalized absolute path from the storage partition root (e.g. "/Download/archive.zip").
    public let path: String
    /// Base file or directory name including extension.
    public let name: String
    /// Specific VFS node type classification.
    public let entryType: AndroidVfsEntryType
    /// Total file size in bytes (0 for directories and symlinks).
    public let sizeBytes: UInt64
    /// Last modification timestamp as seconds since Unix epoch.
    public let modifiedTimestamp: UInt64
    /// Optional 32-bit MTP ObjectHandle if addressed through MTP protocol.
    public let objectHandle: UInt32?
    /// Flag indicating if the entry resides under an Android 11+ scoped storage restricted path.
    public let isRestricted: Bool

    /// Conformance to Identifiable using normalized path.
    public var id: String {
        path
    }

    /// Indicates whether the node represents a directory or restricted directory.
    public var isDirectory: Bool {
        entryType == .directory || entryType == .restrictedDirectory
    }

    /// Indicates whether the node is a regular file.
    public var isFile: Bool {
        entryType == .file
    }

    /// Indicates whether the node is a symbolic link.
    public var isSymlink: Bool {
        entryType == .symlink
    }

    /// Convenience date calculated from Unix modification timestamp.
    public var modifiedDate: Date {
        Date(timeIntervalSince1970: TimeInterval(modifiedTimestamp))
    }

    /// Determines whether a given normalized VFS path falls under Android Scoped Storage restrictions.
    /// - Parameter path: Normalized VFS path string.
    /// - Returns: True if path is restricted under standard MTP access rules.
    public static func isRestrictedPath(_ path: String) -> Bool {
        let normalized = path.hasPrefix("/") ? path : "/" + path
        return normalized.hasPrefix("/Android/data") || normalized.hasPrefix("/Android/obb")
    }

    /// Memberwise initializer for AndroidStorageNode with automatic restriction detection.
    public init(
        path: String,
        name: String,
        entryType: AndroidVfsEntryType,
        sizeBytes: UInt64 = 0,
        modifiedTimestamp: UInt64 = 0,
        objectHandle: UInt32? = nil,
        isRestricted: Bool? = nil
    ) {
        self.path = path
        self.name = name
        self.entryType = entryType
        self.sizeBytes = sizeBytes
        self.modifiedTimestamp = modifiedTimestamp
        self.objectHandle = objectHandle

        if let isRestricted {
            self.isRestricted = isRestricted
        } else {
            self.isRestricted = entryType == .restrictedDirectory || Self.isRestrictedPath(path)
        }
    }
}

// MARK: - UniFFI Type Conversions

extension AndroidVfsEntryType {
    public init(_ ffi: UniFfiVfsEntryType) {
        switch ffi {
        case .file: self = .file
        case .directory: self = .directory
        case .symlink: self = .symlink
        case .restrictedDirectory: self = .restrictedDirectory
        }
    }
}

extension AndroidStorageNode {
    public init(_ ffi: UniFfiAndroidVfsNode) {
        self.init(
            path: ffi.path,
            name: ffi.name,
            entryType: AndroidVfsEntryType(ffi.entryType),
            sizeBytes: ffi.sizeBytes,
            modifiedTimestamp: ffi.modifiedTimestamp,
            objectHandle: ffi.objectHandle,
            isRestricted: ffi.isRestricted
        )
    }
}
