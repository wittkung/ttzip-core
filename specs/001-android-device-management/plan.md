# Implementation Plan: Android Device Direct Connection and File Management

**Branch**: `001-android-device-management` | **Date**: 2026-09-13 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/001-android-device-management/spec.md`

## Summary

This feature provides seamless macOS direct connection, browsing, and file management for Android mobile devices within TTZip. The technical approach implements a **Dual-Mode Dynamic Protocol Stack** in a dedicated Rust crate (`ttzip-device-android`) supporting both standard MTP 1.1 (zero-configuration plug-and-play) and ADB SYNC (high-speed debugging channel penetrating `/Android/data`), with user-space IOKit interface seizing (`USBInterfaceOpenSeize`) to eliminate macOS `PTPCamera` conflicts. Remote storage is presented via a pure in-app VFS Miller columns view, adhering to the **Stream-First Invariant** with zero-download ranged archive inspection (`GetPartialObject64`) and direct pipeline extraction (`archive_read_data_block` pushed directly to USB Bulk OUT).

---

## Technical Context

**Language/Version**: Rust 1.75+ (2021 Edition), Swift 6 (Strict Concurrency Enabled).

**Primary Dependencies**:
- `nusb 0.2.7` (Pure Rust cross-platform USB transport, native IOKit backend on macOS).
- `adb_client 3.2.3` (Pure Rust ADB protocol implementation, TCP/USB transport, and mDNS discovery).
- `mdns-sd 0.19.2` (mDNS zero-configuration network discovery for wireless pairing).
- Mozilla UniFFI 0.28 (Proc-macro cross-language binding generation).
- `libarchive` (C-ABI archive decoding pipeline integration).

**Storage**: In-memory `VfsTree` topology representation; zero intermediate temporary host disk usage (`/tmp`).

**Testing**:
- Targeted Rust unit tests: `cargo test -p ttzip-device-android --lib`.
- Integration test harnesses: `cargo test -p ttzip-device-android --test <test_name>`.
- Swift 6 actor and UI tests: `swift test --filter AndroidDeviceTests`.
- Contract linting: `bash .specify/scripts/bash/lint-contracts.sh specs/001-android-device-management/contracts`.

**Target Platform**: macOS 13.0+ (Ventura, Sonoma, Sequoia) on Apple Silicon (arm64) and Intel (x86_64).

**Project Type**: Systems library (Rust crate) + Swift 6 presentation framework integration.

**Performance Goals**:
- Device sidebar appearance within 2 seconds of USB attachment.
- Traversal and rendering of 10,000 files in under 2 seconds.
- 4GB remote ZIP inspection in $<100\text{ms}$ with 0 bytes local disk write.
- Sequential transfer throughput $\ge 35\text{MB/s}$ (USB 2.0) and $\ge 80\text{MB/s}$ (USB 3.0).
- Gallery index refresh $<1$ second after media file writes.

**Constraints**:
- Host resident RAM ceiling $\le 64\text{MB}$ during active streaming jobs.
- Zero kernel extensions (Zero-KEXT) and zero privileged helper daemons.
- Single-file LOC limit $\le 800$ LOC (target mean $\le 350$ LOC).
- Absolute zero compiler warnings under `-D warnings` and `-warnings-as-errors`.

**Scale/Scope**: Support single or multiple connected Android devices (USB or Wi-Fi); storage volumes exceeding 1TB; directory hierarchies with over 50,000 nodes.

---

## Constitution Check

*GATE: All core architectural invariants must pass.*

| Invariant / Mandate | Status | Verification & Compliance Strategy |
| :--- | :--- | :--- |
| **1. 100% Mozilla UniFFI Mandatory Standard** | **PASS** | `ttzip-device-android` exports all types via `#[uniffi::export]` and `#[derive(uniffi::Record)]`. Zero manual C-ABI裸 pointers. Zero subprocess invocation (`Process`/`Command` forbidden). |
| **2. Swift 6 Presentation Boundary** | **PASS** | Swift layer contains exclusively SwiftUI views (`AndroidMillerView`), `@Observable` view models, and `AndroidDeviceManager` Actor. All USB I/O, socket parsing, and VFS construction reside in Rust. |
| **3. Strict Single-File LOC ($\le 800$ LOC)** | **PASS** | Crate is decoupled into discrete orthogonal submodules: `transport/usb_iokit.rs`, `transport/tcp_client.rs`, `mtp/protocol.rs`, `mtp/operations.rs`, `adb/protocol.rs`, `adb/sync_service.rs`. Each file is $<400$ LOC. |
| **4. Zero In-Tree Path Invariant** | **PASS** | Dynamic library loading and headers follow UniFFI self-contained bundle conventions without hardcoded relative paths. |
| **5. Stream-First Invariant** | **PASS** | 4GB archive inspection uses `GetPartialObject64` (transfers $<300\text{KB}$). Direct extraction pushes unpacked blocks directly to USB Bulk OUT without host `/tmp` staging. Buffer sizes clamped at 64KB micro-buffers. |
| **6. Invariant-First (Zip-Slip Defense)** | **PASS** | All remote write paths pass through `path_sanitizer.rs`. Integer calculations use overflow-checked arithmetic. |
| **7. Bounds-First (Sensitive Memory)** | **PASS** | Wireless pairing tokens and authentication keys are zeroized on drop. |
| **8. Real Research & Zero Hallucination** | **PASS** | All technology choices verified via actual docs.rs, macOS SDK headers, and AOSP source inspection, with explicit Source links recorded in `research.md`. |

---

## Project Structure

### Documentation (this feature)

```text
specs/001-android-device-management/
├── plan.md              # This implementation plan
├── research.md          # Sourced research and architectural decisions
├── data-model.md        # Entities, enums, state machines, validation rules
├── quickstart.md        # End-to-end validation scenarios and execution commands
├── contracts/           # Validated JSON schemas and interface contracts
│   ├── device_android_api.json
│   └── device_android_transfer.json
└── checklists/
    └── requirements.md  # Specification quality validation checklist
```

### Source Code Layout

```text
core/
├── rust/
│   ├── crates/
│   │   └── ttzip-device-android/           # [NEW] Pure Rust hardware communication crate
│   │       ├── Cargo.toml
│   │       └── src/
│   │           ├── lib.rs                  # DeviceStorageDriver trait & module exports
│   │           ├── error.rs                # Strongly typed DeviceError enum
│   │           ├── models.rs               # AndroidDevice, AndroidVfsNode, TransferJob
│   │           ├── traits.rs               # DeviceStorageDriver async trait
│   │           ├── manager.rs              # DeviceManager with protocol elevation
│   │           ├── transport/
│   │           │   ├── mod.rs
│   │           │   ├── usb_iokit.rs        # nusb / IOKit USB transport with Seize claim
│   │           │   ├── tcp_client.rs       # ADB TCP & Wireless TLS 1.3 transport
│   │           │   ├── hotplug.rs          # Event-driven IOKit RunLoop hotplug listener
│   │           │   ├── mdns.rs             # mDNS service discovery (mdns-sd 0.19.2)
│   │           │   └── recovery.rs         # 4-step pipe stall recovery pipeline
│   │           ├── mtp/
│   │           │   ├── mod.rs
│   │           │   ├── protocol.rs         # MTP 1.1 container codecs & opcodes
│   │           │   ├── operations.rs       # GetPartialObject64, SendObjectInfo, SendObject
│   │           │   └── driver.rs           # MtpDeviceDriver implementing DeviceStorageDriver
│   │           └── adb/
│   │               ├── mod.rs
│   │               ├── protocol.rs         # amessage state machine & CNXN handshake
│   │               ├── sync_service.rs     # SYNC protocol (LIST, STAT, RECV, SEND, scan_file)
│   │               ├── driver.rs           # AdbDeviceDriver with Scoped Storage bypass
│   │               └── wireless_pairing.rs # TLS 1.3 SPAKE2 6-digit PIN pairing engine
│   └── ttzip-engine/
│       └── src/
│           ├── archive/source/
│           │   └── device_source.rs        # [NEW] Bridges DeviceStorageDriver to ArchiveSource
│           ├── pipeline/
│           │   └── device_sink.rs          # [NEW] Direct streaming extraction sink to device
│           └── uniffi_api/
│               └── device_android/         # [NEW] Modular UniFFI proc-macro bindings
│                   ├── mod.rs              # Submodule declarations and re-exports
│                   ├── types.rs            # UniFFI record and enum models
│                   ├── registry.rs         # Device registration and driver storage
│                   ├── pipeline.rs         # uniffi_extract_to_device, download, upload
│                   ├── inspection.rs       # uniffi_inspect_remote_archive
│                   ├── wireless.rs         # uniffi_pair_wireless_device
│                   └── hotplug.rs          # uniffi_start_hotplug_monitoring callback bridge
└── Sources/TTZipCore/
    └── Devices/                            # [NEW] Swift 6 facade & Actor state machine
        ├── AndroidDeviceManager.swift      # Device discovery Actor & lifecycle coordinator
        ├── AndroidDevice.swift             # Thread-safe device model
        ├── AndroidStorageNode.swift        # Adapts remote nodes to ArchiveTreeNode
        └── TransferJobCoordinator.swift    # Background transfer runner & progress tracker

apple/
└── Sources/TTZipApp/
    ├── ViewModels/
    │   └── AndroidDeviceViewModel.swift    # @Observable presentation view model
    └── Views/
        ├── Sidebar/
        │   ├── FinderFavoritesSidebarView.swift # Exposes Android devices under "Locations"
        │   └── WirelessDeviceDiscoveryView.swift# Nearby Wi-Fi device discovery panel
        ├── Components/
        │   └── TransferProgressHUD.swift       # Floating transfer progress card
        └── Explorer/
            ├── AndroidMillerView.swift         # Multi-column Miller browsing view
            ├── AndroidHeaderView.swift         # Breadcrumb navigation and partition picker
            ├── ScopedStorageNoticeView.swift   # Inline lock badge guidance
            ├── AdbEnableGuideSheet.swift       # Modal drawer for enabling USB debugging
            └── AndroidPairingSheet.swift       # 6-digit PIN pairing modal sheet
```

---

## Complexity Tracking

| Component / Pattern | Why Needed | Simpler Alternative Rejected Because |
| :--- | :--- | :--- |
| **Dual-Mode Stack (MTP + ADB)** | Casual users need zero-config MTP; power users need `/Android/data` access via ADB. | Single MTP is blocked on Android 11+ `/Android/data`; single ADB requires developer mode for all users. |
| **IOKit `USBInterfaceOpenSeize`** | macOS `PTPCamera` holds exclusive lock on USB Class 0x06 devices. | `killall -9 PTPCamera` leaves device USB endpoints stalled and causes `DeviceBusy`. |
| **Pure In-App VFS** | Native macOS Miller columns browsing without system-level file system interference. | macFUSE requires reducing system security; Apple FileProvider deadlocks on abrupt USB pull; NFS suffers from QuickLook prefetch storms. |
