// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import SwiftUI

/// Unified text field modifier for TTZip UI design system.
public struct TTZipTextFieldModifier: ViewModifier {
    public let cornerRadius: CGFloat

    public init(cornerRadius: CGFloat = 8) {
        self.cornerRadius = cornerRadius
    }

    public func body(content: Content) -> some View {
        content
            .padding(.horizontal, 10)
            .padding(.vertical, 7)
            .background(Color.primary.opacity(0.04))
            .clipShape(RoundedRectangle(cornerRadius: cornerRadius))
            .overlay(
                RoundedRectangle(cornerRadius: cornerRadius)
                    .strokeBorder(Color.primary.opacity(0.1), lineWidth: 0.5)
            )
    }
}

extension View {
    public func ttzipTextFieldStyle(cornerRadius: CGFloat = 8) -> some View {
        self.modifier(TTZipTextFieldModifier(cornerRadius: cornerRadius))
    }
}
