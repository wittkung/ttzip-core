// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// Value type representing a reusable user-defined compression preset configuration.
public struct CompressionPreset: Identifiable, Codable, Equatable, Sendable {
    public let id: UUID
    public var name: String
    public var format: ArchiveCompressionFormat
    public var level: ArchiveCompressionLevel
    public var splitVolumeSizeBytes: Int64? // nil means no multi-volume split (e.g. 20 * 1024 * 1024 * 1024 for 20GB)
    public var defaultPassword: String?
    public var skipMacJunk: Bool
    public var skipGitDirectory: Bool
    
    public init(
        id: UUID = UUID(),
        name: String,
        format: ArchiveCompressionFormat,
        level: ArchiveCompressionLevel,
        splitVolumeSizeBytes: Int64? = nil,
        defaultPassword: String? = nil,
        skipMacJunk: Bool = true,
        skipGitDirectory: Bool = false
    ) {
        self.id = id
        self.name = name
        self.format = format
        self.level = level
        self.splitVolumeSizeBytes = splitVolumeSizeBytes
        self.defaultPassword = defaultPassword
        self.skipMacJunk = skipMacJunk
        self.skipGitDirectory = skipGitDirectory
    }
    
    public var splitVolumeDescription: String {
        localizedSplitVolumeDescription()
    }
    
    public func localizedSplitVolumeDescription(language: AppLanguage? = nil) -> String {
        guard let bytes = splitVolumeSizeBytes, bytes > 0 else {
            return TTZipLocalizationManager.shared.string(for: L10n.Compress.splitVolumeNone, language: language)
        }
        let targetLang = language ?? TTZipLocalizationManager.shared.currentLanguage
        return ByteSizeFormatter.format(bytes: bytes, style: .binaryIEC, language: targetLang)
    }

    /// Validates this preset against the Rust microkernel capability matrix.
    public func validate() throws {
        try format.validateConfiguration(
            level: level.rawValue,
            hasPassword: defaultPassword != nil && !(defaultPassword?.isEmpty ?? true),
            isSplit: (splitVolumeSizeBytes ?? 0) > 0
        )
    }
}

// MARK: - PrototypeCopyable Prototype Pattern Extension
extension CompressionPreset: PrototypeCopyable {
    /// Creates an independent clone with a new UUID.
    public func clone() -> CompressionPreset {
        return clone(newId: UUID(), newName: nil)
    }
    
    /// Prototype copy with custom ID and optional new name.
    public func clone(newId: UUID = UUID(), newName: String? = nil) -> CompressionPreset {
        return CompressionPreset(
            id: newId,
            name: newName ?? self.name,
            format: self.format,
            level: self.level,
            splitVolumeSizeBytes: self.splitVolumeSizeBytes,
            defaultPassword: self.defaultPassword,
            skipMacJunk: self.skipMacJunk,
            skipGitDirectory: self.skipGitDirectory
        )
    }
}

// MARK: - Preset Manager

/// Persistence and UI presentation coordinator for compression presets.
///
/// Delegates all core capability verification and preset tuning to the Rust microkernel.
public final class PresetManager: @unchecked Sendable {
    public static let shared = PresetManager()
    
    private let userDefaults: UserDefaults
    private let storageKey: String
    private var cachedPresets: [CompressionPreset] = []
    private let lock = NSLock()
    
    public init(
        userDefaults: UserDefaults = .standard,
        storageKey: String = "TTZip_User_Compression_Presets_v3"
    ) {
        self.userDefaults = userDefaults
        self.storageKey = storageKey
        loadPresets()
    }
    
    public var presets: [CompressionPreset] {
        lock.withLock {
            cachedPresets
        }
    }
    
    public func preset(for id: UUID) -> CompressionPreset? {
        lock.withLock {
            cachedPresets.first(where: { $0.id == id })
        }
    }
    
    public func loadPresets() {
        lock.withLock {
            if let data = userDefaults.data(forKey: storageKey),
               let list = try? JSONDecoder().decode([CompressionPreset].self, from: data),
               !list.isEmpty {
                self.cachedPresets = list
            } else {
                self.cachedPresets = PresetManager.defaultBuiltInPresets
                saveToStorageLocked()
            }
        }
    }
    
    public func savePreset(_ preset: CompressionPreset) {
        lock.withLock {
            if let index = cachedPresets.firstIndex(where: { $0.id == preset.id }) {
                cachedPresets[index] = preset
            } else {
                cachedPresets.append(preset)
            }
            saveToStorageLocked()
        }
    }
    
    public func deletePreset(id: UUID) {
        lock.withLock {
            cachedPresets.removeAll(where: { $0.id == id })
            saveToStorageLocked()
        }
    }
    
    /// Duplicates an existing preset using Prototype Pattern.
    @discardableResult
    public func duplicatePreset(id: UUID, newName: String? = nil) -> CompressionPreset? {
        return lock.withLock {
            guard let source = cachedPresets.first(where: { $0.id == id }) else { return nil }
            let defaultName = newName ?? "\(source.name) (Copy)"
            let item = source.clone(newId: UUID(), newName: defaultName)
            cachedPresets.append(item)
            saveToStorageLocked()
            return item
        }
    }
    
    /// Derives and saves a new preset from a prototype model.
    @discardableResult
    public func createPresetFromPrototype(_ prototype: CompressionPreset, newName: String? = nil) -> CompressionPreset {
        let cloned = lock.withLock { () -> CompressionPreset in
            let item = prototype.clone(newId: UUID(), newName: newName)
            cachedPresets.append(item)
            saveToStorageLocked()
            return item
        }
        return cloned
    }
    
    public func resetToDefaults() {
        lock.withLock {
            cachedPresets = PresetManager.defaultBuiltInPresets
            saveToStorageLocked()
        }
    }

    /// Validates preset configuration using the Rust microkernel capability matrix.
    public func validatePreset(_ preset: CompressionPreset) throws {
        try preset.validate()
    }

    /// Resolves recommended compression level from Rust microkernel for a preset name.
    public func recommendedLevel(for presetName: String, format: ArchiveCompressionFormat) -> ArchiveCompressionLevel {
        let levelInt = getRecommendedPresetLevel(preset: presetName, format: format.uniffiFormat)
        return ArchiveCompressionLevel(rawValue: Int(levelInt)) ?? .normal
    }
    
    private func saveToStorageLocked() {
        if let encoded = try? JSONEncoder().encode(cachedPresets) {
            userDefaults.set(encoded, forKey: storageKey)
        }
    }
    
    public static var defaultBuiltInPresets: [CompressionPreset] {
        return [
            CompressionPreset(
                name: "7Z 20GB",
                format: .sevenZip,
                level: .store,
                splitVolumeSizeBytes: 20 * 1024 * 1024 * 1024,
                defaultPassword: nil,
                skipMacJunk: true
            ),
            CompressionPreset(
                name: "ZIP 25MB",
                format: .zip,
                level: .normal,
                splitVolumeSizeBytes: 25 * 1024 * 1024,
                defaultPassword: nil,
                skipMacJunk: true
            ),
            CompressionPreset(
                name: "7Z Source Package",
                format: .sevenZip,
                level: .normal,
                defaultPassword: nil,
                skipMacJunk: true,
                skipGitDirectory: true
            ),
            CompressionPreset(
                name: "TAR.ZST Fast",
                format: .tarZst,
                level: .ultra,
                defaultPassword: nil,
                skipMacJunk: true
            )
        ]
    }
}

