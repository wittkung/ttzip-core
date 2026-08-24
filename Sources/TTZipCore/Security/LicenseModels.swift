// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// Supported license tiers for TTZip.
public enum LicenseTier: String, Codable, Sendable {
    case proLifetime = "pro_lifetime"
    case proBusiness = "pro_business"
}

/// Structured payload contained within an Ed25519 signed license key.
public struct LicensePayload: Codable, Sendable, Equatable {
    public let v: Int
    public let email: String
    public let tier: LicenseTier
    public let issued_at: String
    public let order_id: String
    
    public init(v: Int = 1, email: String, tier: LicenseTier = .proLifetime, issued_at: String, order_id: String) {
        self.v = v
        self.email = email
        self.tier = tier
        self.issued_at = issued_at
        self.order_id = order_id
    }
}

/// Comprehensive channel and license authorization state.
public enum ChannelLicenseState: Sendable, Equatable {
    case community                           // Free & Open-Source Community Build (100% full features)
    case directPro(payload: LicensePayload)  // Direct release build with verified Ed25519 license key
    case masPro                              // Mac App Store paid upfront build
    case steamPro                            // Steam Store paid upfront build
    
    public var isPro: Bool {
        switch self {
        case .community:
            return false
        case .directPro, .masPro, .steamPro:
            return true
        }
    }
    
    public var badgeTitle: String {
        switch self {
        case .community:
            return "Community Edition"
        case .directPro:
            return "Pro Lifetime (Direct)"
        case .masPro:
            return "Pro Lifetime (App Store)"
        case .steamPro:
            return "Pro Lifetime (Steam)"
        }
    }
    
    public var shortDescription: String {
        switch self {
        case .community:
            return "Open source build with unrestricted core functionality."
        case .directPro(let payload):
            return "Registered to \(payload.email) (Order: \(payload.order_id))"
        case .masPro:
            return "Licensed via Mac App Store with automated App Sandbox protection."
        case .steamPro:
            return "Licensed via Steam Store with zero-configuration DRM-Free activation."
        }
    }
}
