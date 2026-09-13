// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
import Foundation
@testable import TTZipCore

final class AndroidDeviceManagerTests: XCTestCase {

    // MARK: - Test Helpers

    private nonisolated static func makeMockDevice(
        id: String,
        name: String = "Test Device",
        vendorId: UInt16 = 0x18D1, // Google
        productId: UInt16 = 0x4EE1, // Nexus/Pixel MTP
        serial: String = "TEST_SERIAL_12345",
        connectionType: AndroidConnectionType = .usbMtp,
        status: AndroidDeviceStatus = .connected,
        partitions: [AndroidStoragePartition] = []
    ) -> AndroidDevice {
        AndroidDevice(
            deviceId: id,
            displayName: name,
            vendorId: vendorId,
            productId: productId,
            serialNumber: serial,
            connectionType: connectionType,
            status: status,
            storagePartitions: partitions
        )
    }

    private nonisolated static func makeMockPartition(
        id: String = "storage_internal_0",
        name: String = "Internal Storage",
        totalBytes: UInt64 = 128_000_000_000,
        availableBytes: UInt64 = 64_000_000_000,
        rootPath: String = "/",
        isRemovable: Bool = false
    ) -> AndroidStoragePartition {
        AndroidStoragePartition(
            partitionId: id,
            displayName: name,
            totalBytes: totalBytes,
            availableBytes: availableBytes,
            rootPath: rootPath,
            isRemovable: isRemovable
        )
    }

    // MARK: - 1. Registration & Querying Tests

    func testDeviceRegistrationAndRetrieval() async {
        let manager = AndroidDeviceManager()
        let partition = Self.makeMockPartition()
        let device1 = Self.makeMockDevice(id: "usb:001", name: "Pixel 8 Pro", partitions: [partition])
        let device2 = Self.makeMockDevice(id: "usb:002", name: "Galaxy S24")

        await manager.registerDevice(device1)
        await manager.registerDevice(device2)

        let allDevices = await manager.getDevices()
        XCTAssertEqual(allDevices.count, 2)
        XCTAssertEqual(allDevices[0].deviceId, "usb:001")
        XCTAssertEqual(allDevices[1].deviceId, "usb:002")

        let retrieved1 = await manager.getDevice(id: "usb:001")
        XCTAssertNotNil(retrieved1)
        XCTAssertEqual(retrieved1?.displayName, "Pixel 8 Pro")
        XCTAssertEqual(retrieved1?.storagePartitions.count, 1)
        XCTAssertEqual(retrieved1?.storagePartitions.first?.displayName, "Internal Storage")

        let nonExistent = await manager.getDevice(id: "usb:non_existent")
        XCTAssertNil(nonExistent)
    }

    // MARK: - 2. Removal Tests

    func testDeviceRemoval() async {
        let manager = AndroidDeviceManager()
        let device = Self.makeMockDevice(id: "usb:remove_target", name: "Removable Device")

        await manager.registerDevice(device)
        let initialDevices = await manager.getDevices()
        XCTAssertEqual(initialDevices.count, 1)

        let removed = await manager.removeDevice(id: "usb:remove_target")
        XCTAssertNotNil(removed)
        XCTAssertEqual(removed?.deviceId, "usb:remove_target")

        let afterRemoval = await manager.getDevices()
        XCTAssertTrue(afterRemoval.isEmpty)

        // Removing non-existent device returns nil
        let removedAgain = await manager.removeDevice(id: "usb:remove_target")
        XCTAssertNil(removedAgain)
    }

    // MARK: - 3. Status and Partition Mutation Tests

    func testDeviceStatusAndPartitionUpdates() async {
        let manager = AndroidDeviceManager()
        let device = Self.makeMockDevice(id: "usb:mutate_test", status: .connecting)

        await manager.registerDevice(device)

        let updatedStatusSuccess = await manager.updateDeviceStatus(id: "usb:mutate_test", status: .connected)
        XCTAssertTrue(updatedStatusSuccess)

        let updatedDevice = await manager.getDevice(id: "usb:mutate_test")
        XCTAssertEqual(updatedDevice?.status, .connected)
        XCTAssertTrue(updatedDevice?.isConnected == true)
        XCTAssertTrue(updatedDevice?.status.isOperational == true)

        let newPartitions = [
            Self.makeMockPartition(id: "part_1", name: "Internal", totalBytes: 100, availableBytes: 50),
            Self.makeMockPartition(id: "part_2", name: "SD Card", totalBytes: 200, availableBytes: 150, isRemovable: true)
        ]
        let updatedPartitionsSuccess = await manager.updateDevicePartitions(id: "usb:mutate_test", partitions: newPartitions)
        XCTAssertTrue(updatedPartitionsSuccess)

        let refreshed = await manager.getDevice(id: "usb:mutate_test")
        XCTAssertEqual(refreshed?.storagePartitions.count, 2)
        XCTAssertEqual(refreshed?.totalCapacityBytes, 300)
        XCTAssertEqual(refreshed?.totalAvailableBytes, 200)

        // Non-existent updates fail gracefully
        let nonExistentStatus = await manager.updateDeviceStatus(id: "usb:unknown", status: .error)
        XCTAssertFalse(nonExistentStatus)
        let nonExistentPartitions = await manager.updateDevicePartitions(id: "usb:unknown", partitions: [])
        XCTAssertFalse(nonExistentPartitions)
    }

    // MARK: - 4. Concurrent Registration & Read Safety

    func testConcurrentRegistrationAndReads() async {
        let manager = AndroidDeviceManager()
        let taskCount = 20

        await withTaskGroup(of: Void.self) { group in
            for i in 0..<taskCount {
                group.addTask {
                    let dev = Self.makeMockDevice(
                        id: String(format: "usb:%04d", i),
                        name: "Concurrent Device \(i)"
                    )
                    await manager.registerDevice(dev)
                    _ = await manager.getDevices()
                    _ = await manager.getDevice(id: String(format: "usb:%04d", i))
                }
            }
        }

        let finalCount = await manager.getDevices().count
        XCTAssertEqual(finalCount, taskCount)
    }

    // MARK: - 5. AsyncStream Broadcast Tests

    func testAsyncStreamInitialYieldAndBroadcast() async {
        let manager = AndroidDeviceManager()
        let initialDevice = Self.makeMockDevice(id: "usb:stream_init", name: "Initial Device")
        await manager.registerDevice(initialDevice)

        let updatesStream = await manager.deviceUpdates

        let expectationReceivedInitial = expectation(description: "Received initial device snapshot")
        let expectationReceivedUpdate = expectation(description: "Received updated device snapshot")

        let streamTask = Task {
            var updateIndex = 0
            for await devices in updatesStream {
                if updateIndex == 0 {
                    XCTAssertEqual(devices.count, 1)
                    XCTAssertEqual(devices.first?.deviceId, "usb:stream_init")
                    expectationReceivedInitial.fulfill()
                } else if updateIndex == 1 {
                    XCTAssertEqual(devices.count, 2)
                    expectationReceivedUpdate.fulfill()
                    break
                }
                updateIndex += 1
            }
        }

        await fulfillment(of: [expectationReceivedInitial], timeout: 2.0)

        let newDevice = Self.makeMockDevice(id: "usb:stream_second", name: "Second Device")
        await manager.registerDevice(newDevice)

        await fulfillment(of: [expectationReceivedUpdate], timeout: 2.0)
        streamTask.cancel()
    }

    func testAsyncStreamMultipleSubscribers() async {
        let manager = AndroidDeviceManager()
        let stream1 = await manager.deviceUpdates
        let stream2 = await manager.deviceUpdates

        let exp1 = expectation(description: "Subscriber 1 received update")
        let exp2 = expectation(description: "Subscriber 2 received update")

        let task1 = Task {
            for await devices in stream1 {
                if devices.contains(where: { $0.deviceId == "usb:multi_sub" }) {
                    exp1.fulfill()
                    break
                }
            }
        }

        let task2 = Task {
            for await devices in stream2 {
                if devices.contains(where: { $0.deviceId == "usb:multi_sub" }) {
                    exp2.fulfill()
                    break
                }
            }
        }

        let device = Self.makeMockDevice(id: "usb:multi_sub", name: "Multi Subscriber Target")
        await manager.registerDevice(device)

        await fulfillment(of: [exp1, exp2], timeout: 2.0)
        task1.cancel()
        task2.cancel()
    }

    // MARK: - 6. Storage Partition Calculations

    func testPartitionCalculationsAndProperties() {
        let partition = AndroidStoragePartition(
            partitionId: "mtp_storage_10001",
            displayName: "SanDisk Extreme",
            totalBytes: 1_000_000,
            availableBytes: 400_000,
            rootPath: "/storage/sdcard1",
            isRemovable: true
        )

        XCTAssertEqual(partition.id, "mtp_storage_10001")
        XCTAssertEqual(partition.usedBytes, 600_000)
        XCTAssertEqual(partition.usageRatio, 0.6, accuracy: 0.0001)
        XCTAssertTrue(partition.isRemovable)

        let emptyPartition = AndroidStoragePartition(
            partitionId: "empty",
            displayName: "Empty",
            totalBytes: 0,
            availableBytes: 0
        )
        XCTAssertEqual(emptyPartition.usedBytes, 0)
        XCTAssertEqual(emptyPartition.usageRatio, 0.0)
    }

    // MARK: - 7. Storage Node Properties & Restricted Directory Invariants

    func testStorageNodePropertiesAndRestrictedDetection() {
        // Standard file
        let fileNode = AndroidStorageNode(
            path: "/Download/document.pdf",
            name: "document.pdf",
            entryType: .file,
            sizeBytes: 1024,
            modifiedTimestamp: 1700000000,
            objectHandle: 42
        )
        XCTAssertEqual(fileNode.id, "/Download/document.pdf")
        XCTAssertTrue(fileNode.isFile)
        XCTAssertFalse(fileNode.isDirectory)
        XCTAssertFalse(fileNode.isSymlink)
        XCTAssertFalse(fileNode.isRestricted)
        XCTAssertEqual(fileNode.objectHandle, 42)

        // Android 11+ Scoped Storage restricted path detection
        let restrictedDataNode = AndroidStorageNode(
            path: "/Android/data/com.example.app/files",
            name: "files",
            entryType: .directory
        )
        XCTAssertTrue(restrictedDataNode.isDirectory)
        XCTAssertTrue(restrictedDataNode.isRestricted)

        let restrictedObbNode = AndroidStorageNode(
            path: "Android/obb/com.example.game",
            name: "com.example.game",
            entryType: .directory
        )
        XCTAssertTrue(restrictedObbNode.isRestricted)

        // Explicit restricted directory entry type
        let explicitRestrictedNode = AndroidStorageNode(
            path: "/custom/restricted",
            name: "restricted",
            entryType: .restrictedDirectory
        )
        XCTAssertTrue(explicitRestrictedNode.isRestricted)
        XCTAssertTrue(explicitRestrictedNode.isDirectory)

        // Static path check verification
        XCTAssertTrue(AndroidStorageNode.isRestrictedPath("/Android/data"))
        XCTAssertTrue(AndroidStorageNode.isRestrictedPath("/Android/obb"))
        XCTAssertTrue(AndroidStorageNode.isRestrictedPath("Android/data/foo"))
        XCTAssertFalse(AndroidStorageNode.isRestrictedPath("/Download"))
        XCTAssertFalse(AndroidStorageNode.isRestrictedPath("/DCIM/Camera"))
    }

    // MARK: - 8. JSON Schema & Codable Contract Round-Trip

    func testDeviceApiSchemaCodableRoundTrip() throws {
        let partition = AndroidStoragePartition(
            partitionId: "mtp_0x00010001",
            displayName: "Internal Shared Storage",
            totalBytes: 256_000_000_000,
            availableBytes: 128_000_000_000,
            rootPath: "/",
            isRemovable: false
        )
        let device = AndroidDevice(
            deviceId: "usb:18d1:4ee1:ABCDEF012345",
            displayName: "Google Pixel 8",
            vendorId: 0x18D1,
            productId: 0x4EE1,
            serialNumber: "ABCDEF012345",
            connectionType: .usbMtp,
            status: .connected,
            storagePartitions: [partition]
        )

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let encodedData = try encoder.encode(device)

        let decoder = JSONDecoder()
        let decodedDevice = try decoder.decode(AndroidDevice.self, from: encodedData)

        XCTAssertEqual(decodedDevice.deviceId, device.deviceId)
        XCTAssertEqual(decodedDevice.displayName, device.displayName)
        XCTAssertEqual(decodedDevice.vendorId, device.vendorId)
        XCTAssertEqual(decodedDevice.productId, device.productId)
        XCTAssertEqual(decodedDevice.serialNumber, device.serialNumber)
        XCTAssertEqual(decodedDevice.connectionType, .usbMtp)
        XCTAssertEqual(decodedDevice.status, .connected)
        XCTAssertEqual(decodedDevice.storagePartitions.count, 1)
        XCTAssertEqual(decodedDevice.storagePartitions.first?.partitionId, "mtp_0x00010001")
        XCTAssertEqual(decodedDevice.storagePartitions.first?.displayName, "Internal Shared Storage")
        XCTAssertEqual(decodedDevice.storagePartitions.first?.totalBytes, 256_000_000_000)
        XCTAssertEqual(decodedDevice.storagePartitions.first?.availableBytes, 128_000_000_000)

        // StorageNode round trip
        let node = AndroidStorageNode(
            path: "/Download/test.zip",
            name: "test.zip",
            entryType: .file,
            sizeBytes: 2048,
            modifiedTimestamp: 1726000000,
            objectHandle: 101,
            isRestricted: false
        )
        let nodeData = try encoder.encode(node)
        let decodedNode = try decoder.decode(AndroidStorageNode.self, from: nodeData)
        XCTAssertEqual(decodedNode.path, node.path)
        XCTAssertEqual(decodedNode.name, node.name)
        XCTAssertEqual(decodedNode.entryType, .file)
        XCTAssertEqual(decodedNode.sizeBytes, 2048)
        XCTAssertEqual(decodedNode.modifiedTimestamp, 1726000000)
        XCTAssertEqual(decodedNode.objectHandle, 101)
        XCTAssertEqual(decodedNode.isRestricted, false)
    }
}
