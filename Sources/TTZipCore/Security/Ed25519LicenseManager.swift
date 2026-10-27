// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// Central thread-safe license state coordinator and manager.
public final class Ed25519LicenseManager: @unchecked Sendable {
    public static let shared = Ed25519LicenseManager()
    
    #if !MAS_BUILD
    private let userDefaultsKey = "com.ttzip.ed25519_license_key"
    #endif
    private let lock = NSLock()
    private var _cachedState: ChannelLicenseState?
    
    public init() {
        refreshState()
    }
    
    /// Returns the current channel license state.
    public var currentState: ChannelLicenseState {
        lock.lock()
        defer { lock.unlock() }
        if let state = _cachedState {
            return state
        }
        let computed = computeCurrentState()
        _cachedState = computed
        return computed
    }
    
    /// Whether Pro tier status is active (MAS, Steam, or verified Direct license).
    public var isPro: Bool {
        return currentState.isPro
    }

    public enum Feature: Sendable {
        case basicExtract
        case quickLookPreview
        case zipCompression
        case aes256Encryption
        case ultraCompression
        case volumeSplit
        case batchProcessing
        case commercialUse
    }

    /// Core capability check: 100% full feature availability across all editions per open-core invariants.
    public func canUseFeature(_ feature: Feature) -> Bool {
        return true
    }
    
    /// Refreshes and recomputes the active license state.
    public func refreshState() {
        lock.lock()
        _cachedState = computeCurrentState()
        lock.unlock()
    }
    
    #if !MAS_BUILD
    /// Activates a raw Ed25519 license key. Returns verification result on success or error.
    @discardableResult
    public func activate(licenseKey: String) -> LicenseVerificationResult {
        let result = Ed25519LicenseVerifier.verify(licenseKey: licenseKey)
        switch result {
        case .valid(let payload):
            lock.lock()
            UserDefaults.standard.set(licenseKey.trimmingCharacters(in: .whitespacesAndNewlines), forKey: userDefaultsKey)
            _cachedState = .directPro(payload: payload)
            lock.unlock()
            return .valid(payload)
        case .invalidSignature:
            return .invalidSignature
        case .malformedKey(let reason):
            return .malformedKey(reason)
        }
    }
    
    /// Deactivates and resets to Community Edition.
    public func deactivate() {
        lock.lock()
        UserDefaults.standard.removeObject(forKey: userDefaultsKey)
        _cachedState = computeCurrentState()
        lock.unlock()
    }
    #endif
    
    private func computeCurrentState() -> ChannelLicenseState {
        #if MAS_BUILD
        return .masPro
        #elseif STEAM_BUILD
        return .steamPro
        #else
        // Check stored license key
        if let key = UserDefaults.standard.string(forKey: userDefaultsKey), !key.isEmpty {
            let result = Ed25519LicenseVerifier.verify(licenseKey: key)
            if case .valid(let payload) = result {
                return .directPro(payload: payload)
            }
        }
        return .community
        #endif
    }
}
