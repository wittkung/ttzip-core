// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import Foundation

/// Prototype Pattern: Standard interface for deep-copying configurations and component state.
public protocol PrototypeCopyable {
    /// Creates and returns an independent cloned copy of the receiver.
    func clone() -> Self
}
