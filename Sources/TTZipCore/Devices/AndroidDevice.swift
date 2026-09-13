// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// Supported communication protocol channels for connected Android devices.
public enum AndroidConnectionType: String, Sendable, Hashable, Codable, CaseIterable, CustomStringConvertible {
    /// Standard user-space USB Media Transfer Protocol (MTP 1.1) interface.
    case usbMtp = "UsbMtp"
    /// High-speed ADB daemon transport over physical USB cable.
    case usbAdb = "UsbAdb"
    /// High-speed ADB transport over local Wi-Fi / TCP network socket.
    case wirelessAdb = "WirelessAdb"

    public var description: String {
        rawValue
    }
}

/// Lifecycle connection and recovery states of an Android hardware interface.
public enum AndroidDeviceStatus: String, Sendable, Hashable, Codable, CaseIterable, CustomStringConvertible {
    /// Initial discovery or connection handshake in progress.
    case connecting = "Connecting"
    /// Actively claiming exclusive USB interface ownership via IOKit USBInterfaceOpenSeize.
    case seizingInterface = "SeizingInterface"
    /// Fully connected and operational for VFS queries and file streaming.
    case connected = "Connected"
    /// Hardware endpoint FIFO overrun or protocol stall undergoing 4-step recovery.
    case stalled = "Stalled"
    /// Hardware interface detached or session terminated.
    case disconnected = "Disconnected"
    /// Fatal unrecoverable communication failure.
    case error = "Error"

    public var description: String {
        rawValue
    }

    /// Indicates whether the device is currently in an active and usable operational state.
    public var isOperational: Bool {
        self == .connected
    }

    /// Indicates whether the device status represents a terminated or failed state.
    public var isTerminal: Bool {
        self == .disconnected || self == .error
    }
}

/// Logical storage volume reported by the Android device (e.g. Internal Storage, Removable MicroSD).
public struct AndroidStoragePartition: Identifiable, Sendable, Hashable, Codable {
    /// Unique partition identifier (e.g., MTP 32-bit StorageID string or mount path).
    public let partitionId: String
    /// Human-readable partition volume label (e.g., "Internal Storage", "SanDisk SD Card").
    public let displayName: String
    /// Total storage capacity of the partition in bytes.
    public let totalBytes: UInt64
    /// Currently available unallocated free space in bytes.
    public let availableBytes: UInt64
    /// Normalized root path within the device VFS (e.g., "/" or "/storage/emulated/0").
    public let rootPath: String
    /// Flag indicating whether this volume is physically removable media.
    public let isRemovable: Bool

    /// Conformance to Identifiable using partitionId.
    public var id: String {
        partitionId
    }

    /// Computed bytes allocated or consumed by existing data.
    public var usedBytes: UInt64 {
        totalBytes >= availableBytes ? totalBytes - availableBytes : 0
    }

    /// Ratio of used storage capacity between 0.0 and 1.0.
    public var usageRatio: Double {
        guard totalBytes > 0 else { return 0.0 }
        return Double(usedBytes) / Double(totalBytes)
    }

    /// Memberwise initializer with sensible defaults.
    public init(
        partitionId: String,
        displayName: String,
        totalBytes: UInt64,
        availableBytes: UInt64,
        rootPath: String = "/",
        isRemovable: Bool = false
    ) {
        self.partitionId = partitionId
        self.displayName = displayName
        self.totalBytes = totalBytes
        self.availableBytes = availableBytes
        self.rootPath = rootPath
        self.isRemovable = isRemovable
    }
}

/// Represents an active or detected Android physical or wireless device connected to the host system.
public struct AndroidDevice: Identifiable, Sendable, Hashable, Codable {
    /// Unique persistent identifier (e.g. "usb:05ac:1234:ABCDEF" or "wifi:192.168.1.50:5555").
    public let deviceId: String
    /// Human-readable device product name (e.g. "Pixel 8 Pro", "Samsung Galaxy S24").
    public var displayName: String
    /// 16-bit USB Vendor ID (or 0 for network-discovered devices).
    public let vendorId: UInt16
    /// 16-bit USB Product ID (or 0 for network-discovered devices).
    public let productId: UInt16
    /// Hardware serial number or ADB device serial string.
    public let serialNumber: String
    /// Protocol channel used for active communication.
    public var connectionType: AndroidConnectionType
    /// Current hardware connection and operational lifecycle state.
    public var status: AndroidDeviceStatus
    /// Array of mounted logical storage partitions available for browsing.
    public var storagePartitions: [AndroidStoragePartition]

    /// Conformance to Identifiable using deviceId.
    public var id: String {
        deviceId
    }

    /// Convenience flag indicating whether the device is in an operational connected state.
    public var isConnected: Bool {
        status == .connected
    }

    /// Aggregate available free bytes across all mounted storage partitions.
    public var totalAvailableBytes: UInt64 {
        storagePartitions.reduce(0) { $0 + $1.availableBytes }
    }

    /// Aggregate total capacity in bytes across all mounted storage partitions.
    public var totalCapacityBytes: UInt64 {
        storagePartitions.reduce(0) { $0 + $1.totalBytes }
    }

    /// Memberwise initializer for AndroidDevice instances.
    public init(
        deviceId: String,
        displayName: String,
        vendorId: UInt16,
        productId: UInt16,
        serialNumber: String,
        connectionType: AndroidConnectionType,
        status: AndroidDeviceStatus,
        storagePartitions: [AndroidStoragePartition] = []
    ) {
        self.deviceId = deviceId
        self.displayName = displayName
        self.vendorId = vendorId
        self.productId = productId
        self.serialNumber = serialNumber
        self.connectionType = connectionType
        self.status = status
        self.storagePartitions = storagePartitions
    }
}

// MARK: - UniFFI Type Conversions

extension AndroidConnectionType {
    public init(_ ffi: UniFfiConnectionType) {
        switch ffi {
        case .usbMtp: self = .usbMtp
        case .usbAdb: self = .usbAdb
        case .wirelessAdb: self = .wirelessAdb
        }
    }
}

extension AndroidDeviceStatus {
    public init(_ ffi: UniFfiDeviceStatus) {
        switch ffi {
        case .connecting: self = .connecting
        case .seizingInterface: self = .seizingInterface
        case .connected: self = .connected
        case .stalled: self = .stalled
        case .disconnected: self = .disconnected
        case .error: self = .error
        }
    }
}

extension AndroidStoragePartition {
    public init(_ ffi: UniFfiStoragePartition) {
        self.init(
            partitionId: ffi.partitionId,
            displayName: ffi.displayName,
            totalBytes: ffi.totalBytes,
            availableBytes: ffi.availableBytes,
            rootPath: ffi.rootPath,
            isRemovable: ffi.isRemovable
        )
    }
}

extension AndroidDevice {
    public init(_ ffi: UniFfiAndroidDevice) {
        self.init(
            deviceId: ffi.deviceId,
            displayName: ffi.displayName,
            vendorId: ffi.vendorId,
            productId: ffi.productId,
            serialNumber: ffi.serialNumber,
            connectionType: AndroidConnectionType(ffi.connectionType),
            status: AndroidDeviceStatus(ffi.status),
            storagePartitions: ffi.storagePartitions.map { AndroidStoragePartition($0) }
        )
    }
}
