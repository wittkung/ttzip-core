// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// Canonical algebraic domain entity classification separating physical storage containers
/// from semantic compound documents, application packages, and media containers.
public enum DocumentEntityTaxonomy: String, Sendable, CaseIterable, Codable {
    /// Navigable archive container of files and directories (e.g. .zip, .7z, .tar, .rar, .cbz, .cbr, .iso).
    case physicalContainer
    
    /// Rich leaf document containing internal XML/HTML/ZIP structures (e.g. .epub, .mobi, .azw3, .fb2, .docx, .xlsx, .pptx).
    case compoundDocument
    
    /// Sandboxed application or bundle package (e.g. .app, .ipa, .apk, .jar).
    case applicationPackage
    
    /// Media container packaging multiplexed video, audio, and subtitle streams (e.g. .mkv, .mp4, .mov, .webm).
    case mediaPackage
    
    /// Regular flat files (e.g. .txt, .pdf, .png, binaries).
    case plainFile

    /// Evaluates whether this item represents an archive container that can be navigated as a directory.
    public var isNavigableContainer: Bool {
        self == .physicalContainer
    }

    /// Evaluates whether default contextual actions should present extraction/unpacking options.
    public var isExtractableContainer: Bool {
        self == .physicalContainer
    }

    /// Classifies a file path or extension into its canonical domain taxonomy.
    public static func classify(extension ext: String, path: String = "") -> DocumentEntityTaxonomy {
        let normalized = ext.lowercased().trimmingCharacters(in: CharacterSet(charactersIn: "."))
        
        switch normalized {
        case "zip", "7z", "tar", "rar", "gz", "bz2", "xz", "zst", "iso", "dmg",
             "wim", "swm", "esd", "cab", "cpio", "ar", "deb", "rpm", "xar", "pkg",
             "squashfs", "sqsh", "lzfse", "lzh", "lha", "zipx", "aea", "aar",
             "cbz", "cbr", "cb7":
            return .physicalContainer
            
        case "epub", "mobi", "azw", "azw3", "fb2", "ibooks", "pdf", "docx", "xlsx", "pptx", "pages", "numbers", "keynote":
            return .compoundDocument
            
        case "app", "ipa", "apk", "jar":
            return .applicationPackage
            
        case "mkv", "mp4", "mov", "webm", "ts", "mts", "m2ts", "avi", "flv":
            return .mediaPackage
            
        default:
            // Handle split archive patterns like .part1.rar, .001, .z01
            if normalized.range(of: #"^\d{3}$"#, options: .regularExpression) != nil ||
               path.contains(".7z.") || path.contains(".zip.") || path.contains(".rar.") ||
               path.contains(".part1.rar") || path.contains(".z01") {
                return .physicalContainer
            }
            return .plainFile
        }
    }
}
