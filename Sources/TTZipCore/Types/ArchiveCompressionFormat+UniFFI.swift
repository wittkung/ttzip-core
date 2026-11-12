// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import TTZipOperationContract

public typealias TtZipArchiveFormat = ArchiveFormat

extension ArchiveCompressionFormat {
    public var uniffiFormat: ArchiveFormat {
        switch self {
        case .sevenZip: return .sevenZip
        case .zip: return .zip
        case .tar: return .tar
        case .tarGz: return .tarGz
        case .tarBz2: return .tarBz2
        case .tarXz: return .tarXz
        case .tarZst: return .tarZstd
        case .tarLz4: return .tarLz4
        case .tarBrotli: return .tarBrotli
        case .tarLzip: return .tarLzip
        case .tarLrzip: return .tarLrzip
        case .iso: return .iso
        case .cab: return .cab
        case .wim: return .wim
        case .dmg: return .dmg
        case .aar: return .aar
        case .cpio: return .cpio
        case .ar: return .ar
        case .deb: return .deb
        case .rpm: return .rpm
        case .xar: return .xar
        case .rar: return .rar
        case .squashfs: return .squashfs
        case .lzfse: return .lzfse
        case .lzh: return .lzh
        case .zst: return .zstd
        case .gz: return .gzip
        case .bz2: return .bzip2
        case .xz: return .xz
        case .lzip: return .lzip
        case .lz4: return .lz4
        case .brotli: return .brotli
        case .snappy: return .snappy
        case .lrzip: return .lrzip
        }
    }

    public var capabilities: UniFfiFormatCapabilities {
        getFormatCapabilities(format: uniffiFormat)
    }

    public var supportsPasswordEncryption: Bool {
        capabilities.supportsPassword
    }

    public var supportsSplitVolume: Bool {
        capabilities.supportsSplitVolume
    }

    public var supportsSolidMode: Bool {
        capabilities.supportsSolidMode
    }

    public var supportedLevels: [ArchiveCompressionLevel] {
        let maxLvl = maxCompressionLevel
        if maxLvl == 0 {
            return [.store]
        } else if self == .zip {
            return [.store, .level1, .level6, .level9, .level12]
        } else {
            return [.store, .level1, .level6, .level9]
        }
    }

    public var minCompressionLevel: Int {
        Int(capabilities.minLevel)
    }

    public var maxCompressionLevel: Int {
        Int(capabilities.maxLevel)
    }

    public var defaultCompressionLevel: Int {
        Int(capabilities.defaultLevel)
    }

    public var validCompressionLevelRange: ClosedRange<Int> {
        minCompressionLevel...maxCompressionLevel
    }

    public func validateConfiguration(level: Int, hasPassword: Bool, isSplit: Bool) throws {
        try validateArchiveParams(
            format: uniffiFormat,
            level: Int32(level),
            hasPassword: hasPassword,
            isSplit: isSplit
        )
    }
}
