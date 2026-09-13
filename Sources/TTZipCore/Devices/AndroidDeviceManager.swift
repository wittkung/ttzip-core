// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// Swift 6 actor-isolated coordinator managing connected Android device state and streaming updates.
public actor AndroidDeviceManager {
    /// Shared singleton instance for unified device lifecycle coordination.
    public static let shared = AndroidDeviceManager()

    /// Registry of connected devices keyed by their unique deviceId.
    private var devices: [String: AndroidDevice] = [:]

    /// Active state stream subscriber continuations keyed by registration UUID.
    private var continuations: [UUID: AsyncStream<[AndroidDevice]>.Continuation] = [:]

    /// Initializes a new independent device manager actor instance.
    public init() {}

    /// Registers or updates an Android device within the manager.
    /// - Parameter device: The device instance to register.
    public func registerDevice(_ device: AndroidDevice) {
        devices[device.deviceId] = device
        broadcastUpdate()
    }

    /// Removes a device by its identifier and broadcasts the change.
    /// - Parameter id: The unique device identifier to remove.
    /// - Returns: The removed device if it previously existed, nil otherwise.
    @discardableResult
    public func removeDevice(id: String) -> AndroidDevice? {
        guard let removed = devices.removeValue(forKey: id) else {
            return nil
        }
        broadcastUpdate()
        return removed
    }

    /// Retrieves an immutable snapshot array of all currently managed devices, ordered deterministically by deviceId.
    /// - Returns: Array of current AndroidDevice instances.
    public func getDevices() -> [AndroidDevice] {
        devices.values.sorted { $0.deviceId < $1.deviceId }
    }

    /// Retrieves a specific device by its unique identifier.
    /// - Parameter id: The unique device identifier.
    /// - Returns: The matching AndroidDevice if registered, nil otherwise.
    public func getDevice(id: String) -> AndroidDevice? {
        devices[id]
    }

    /// Updates the operational status of an existing registered device.
    /// - Parameters:
    ///   - id: The device identifier.
    ///   - status: The new device status to apply.
    /// - Returns: True if the device was found and updated, false otherwise.
    @discardableResult
    public func updateDeviceStatus(id: String, status: AndroidDeviceStatus) -> Bool {
        guard var device = devices[id] else {
            return false
        }
        device.status = status
        devices[id] = device
        broadcastUpdate()
        return true
    }

    /// Updates the storage partitions of an existing registered device.
    /// - Parameters:
    ///   - id: The device identifier.
    ///   - partitions: The refreshed array of storage partitions.
    /// - Returns: True if the device was found and updated, false otherwise.
    @discardableResult
    public func updateDevicePartitions(id: String, partitions: [AndroidStoragePartition]) -> Bool {
        guard var device = devices[id] else {
            return false
        }
        device.storagePartitions = partitions
        devices[id] = device
        broadcastUpdate()
        return true
    }

    /// Clears all registered devices and notifies active stream observers.
    public func removeAllDevices() {
        guard !devices.isEmpty else { return }
        devices.removeAll()
        broadcastUpdate()
    }

    /// Asynchronous stream yielding the complete device list snapshot upon subscription
    /// and broadcasting subsequent updates whenever devices are added, modified, or removed.
    public var deviceUpdates: AsyncStream<[AndroidDevice]> {
        let (stream, continuation) = AsyncStream<[AndroidDevice]>.makeStream()
        let id = UUID()
        continuations[id] = continuation
        continuation.yield(getDevices())
        continuation.onTermination = { @Sendable [weak self] _ in
            Task { [weak self] in
                await self?.removeContinuation(id: id)
            }
        }
        return stream
    }

    /// Internal actor-isolated cleanup handler for terminated stream continuations.
    private func removeContinuation(id: UUID) {
        continuations.removeValue(forKey: id)
    }

    /// Internal helper broadcasting the current device snapshot to all active subscriber continuations.
    private func broadcastUpdate() {
        let currentSnapshot = getDevices()
        for continuation in continuations.values {
            continuation.yield(currentSnapshot)
        }
    }

    // MARK: - Hardware Hotplug Monitoring & Discovery

    private final class HotplugListenerBridge: UniFfiDeviceEventListener, @unchecked Sendable {
        weak var manager: AndroidDeviceManager?

        init(manager: AndroidDeviceManager) {
            self.manager = manager
        }

        func onDevicesChanged() {
            Task { [weak manager] in
                await manager?.refreshUsbDevices()
            }
        }
    }

    /// Flag tracking whether hardware monitoring is currently active.
    private var isHardwareMonitoringActive: Bool = false

    /// Starts event-driven IOKit hotplug monitoring and performs an initial device scan.
    public func startHardwareMonitoring() {
        guard !isHardwareMonitoringActive else { return }
        isHardwareMonitoringActive = true
        let listener = HotplugListenerBridge(manager: self)
        do {
            try uniffiStartHotplugMonitoring(listener: listener)
        } catch {
            isHardwareMonitoringActive = false
        }
        refreshUsbDevices()
    }

    /// Stops event-driven hardware hotplug monitoring.
    public func stopHardwareMonitoring() {
        guard isHardwareMonitoringActive else { return }
        isHardwareMonitoringActive = false
        uniffiStopHotplugMonitoring()
    }

    /// Scans USB bus for attached Android devices and syncs registered devices.
    public func refreshUsbDevices() {
        guard let scanned = try? uniffiScanUsbDevices() else { return }
        var activeIds = Set<String>()
        for ffiDev in scanned {
            let dev = AndroidDevice(ffiDev)
            activeIds.insert(dev.deviceId)
            registerDevice(dev)
        }
        for dev in devices.values where dev.connectionType != .wirelessAdb && !activeIds.contains(dev.deviceId) {
            removeDevice(id: dev.deviceId)
        }
    }
}
