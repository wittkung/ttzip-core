// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import Foundation
import Dispatch

/// Cache entity capable of flushing or trimming memory on demand.
public protocol EphemeralCacheControllable: AnyObject, Sendable {
    func trimMemory()
    func purgeAll()
}

/// Unified memory pressure and ephemeral resource coordinator.
/// Listens to Darwin kernel `DISPATCH_SOURCE_TYPE_MEMORYPRESSURE` events and orchestrates cache evictions within <= 50ms.
public final actor EphemeralResourceBroker {
    public static let shared = EphemeralResourceBroker()
    
    private var registeredCaches: [WeakCacheBox] = []
    private let memoryPressureSource: DispatchSourceMemoryPressure
    
    private final class WeakCacheBox: @unchecked Sendable {
        weak var cache: EphemeralCacheControllable?
        init(_ cache: EphemeralCacheControllable) {
            self.cache = cache
        }
    }
    
    private init() {
        let source = DispatchSource.makeMemoryPressureSource(eventMask: [.warning, .critical], queue: .global(qos: .userInteractive))
        self.memoryPressureSource = source
        source.setEventHandler { [weak self] in
            guard let self = self else { return }
            let event = source.data
            Task {
                if event.contains(.critical) {
                    await self.purgeAllCaches()
                } else if event.contains(.warning) {
                    await self.trimAllCaches()
                }
            }
        }
        source.activate()
    }
    
    /// Registers a controllable cache with the central broker.
    public func register(cache: EphemeralCacheControllable) {
        cleanDeallocatedBoxes()
        registeredCaches.append(WeakCacheBox(cache))
    }
    
    /// Evicts all registered caches immediately upon critical memory pressure.
    public func purgeAllCaches() {
        cleanDeallocatedBoxes()
        for box in registeredCaches {
            box.cache?.purgeAll()
        }
    }
    
    /// Trims memory across all registered caches.
    public func trimAllCaches() {
        cleanDeallocatedBoxes()
        for box in registeredCaches {
            box.cache?.trimMemory()
        }
    }
    
    private func cleanDeallocatedBoxes() {
        registeredCaches.removeAll { $0.cache == nil }
    }
    
    deinit {
        memoryPressureSource.cancel()
    }
}
