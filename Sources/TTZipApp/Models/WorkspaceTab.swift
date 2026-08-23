// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import Foundation

/// Workspace navigation tab classification.
public enum WorkspaceTab: String, CaseIterable, Identifiable, Codable, Sendable {
    case home = "home"
    case compressWorkspace = "compressWorkspace"
    case presets = "presets"
    case benchmark = "benchmark"
    case vault = "vault"
    case settings = "settings"

    public var id: String { rawValue }
}
