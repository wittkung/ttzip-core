// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import AppKit

/// Abstraction interface for GUI desktop file reveal service.
public protocol FileViewerServiceProtocol: Sendable {
    /// Reveals and selects file or folder in macOS Finder.
    func revealInFinder(at path: String)
}

/// macOS NSWorkspace desktop file viewer default implementation.
public final class MacNSWorkspaceFileViewer: FileViewerServiceProtocol {
    public init() {}
    
    public func revealInFinder(at path: String) {
        NSWorkspace.shared.selectFile(path, inFileViewerRootedAtPath: "")
    }
}
