# Feature Specification: Android Device Direct Connection and File Management

**Feature Branch**: `001-android-device-management`

**Created**: 2026-09-13

**Status**: Draft

**Classification**: [Full SDD]

**Input**: User description: "详细调研，我们需要全面支持安卓文件系统的管理，可以通过mac直接连接、访问和管理"

## Clarifications

### Session 2026-09-13
- Q: 第一期（v1.0）是否将 Android 设备连接范围严格收敛于 USB 有线物理直连，将 Wi-Fi 无线调试与局域网配对递延至后续演进版本？ → A: v1.0 同步支持 USB 有线与 Wi-Fi 无线直连（引入局域网 mDNS 服务发现、TLS 1.3 握手与 SPAKE2 配对码配对流程）。
- Q: 在免调试的标准 MTP 模式下，对于 Android 11+ 系统级阻断的应用私有目录（/Android/data 与 /Android/obb），TTZip 界面应采取何种交互反馈策略？ → A: 显示带锁形徽标并提供引导（在米勒列中保留 /Android/data 目录项并标记受限徽标，点击时显示轻量引导提示用户“开启 USB 调试即可一键穿透访问”）。

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Plug-and-Play Device Discovery and Miller Columns Browsing (Priority: P1)

As a Mac user with an Android phone, I want to plug my device via USB, see it instantly appear in the TTZip sidebar under the "Locations" section, and browse its folder structure using native macOS Miller columns without installing any companion app on my phone.

**Why this priority**:
Zero-install, zero-friction connectivity is the primary foundation for device management. Without reliable device detection and directory browsing, all other file operations are inaccessible.

**Independent Test**:
Connect an Android device via USB with standard "File Transfer" (MTP) enabled. Verify the device automatically appears in the TTZip sidebar within 2 seconds. Clicking the device displays the storage roots (e.g., Internal Storage, SD Card) and allows expanding folders across Miller columns with zero system conflict errors.

**Acceptance Scenarios**:
1. **Given** TTZip is running on macOS and an Android device is connected via USB in File Transfer mode, **When** the system detects the hardware, **Then** TTZip claims the interface without "Resource Busy" errors and mounts the device under the sidebar "Locations" section.
2. **Given** an Android device is mounted in TTZip, **When** the user clicks through directories in the Miller columns view, **Then** folder items are populated progressively with accurate file names, sizes, and modification timestamps.
3. **Given** an Android device is disconnected by pulling the USB cable during navigation, **When** the hardware disconnect event fires, **Then** TTZip gracefully cleans up the device session, removes the entry from the sidebar, and returns the workspace to the home view without freezing or crashing.

---

### User Story 2 - Instant Archive Inspection and Direct Pipeline Extraction (Priority: P2)

As a Mac user managing large files, I want to inspect 4GB+ archive files directly on the Android device without downloading the entire file to Mac local disk, and extract Mac local archives directly into Android folders without using intermediate temporary disk space.

**Why this priority**:
Transferring multi-gigabyte files over USB 2.0/3.0 to temporary Mac disk space causes unnecessary wear and long waiting times. Direct stream-first inspection and pipeline extraction provides the core differentiated capability of TTZip.

**Independent Test**:
Select a 4GB ZIP archive located inside the Android device's storage. Double-click to inspect: the archive directory tree displays in under 1 second while Mac disk write remains 0 bytes. Drag a 1GB ZIP from Mac into an Android directory: files extract directly into Android flash storage without inflating Mac `/tmp` disk usage.

**Acceptance Scenarios**:
1. **Given** a 4GB+ ZIP file on Android storage, **When** the user opens it in TTZip, **Then** TTZip reads only the central directory headers from the tail of the remote file and displays the virtual directory tree in under 100 milliseconds.
2. **Given** a multi-file archive on Mac local storage, **When** the user drops it into an Android target folder, **Then** the decompression stream directly writes unpacked entries into the device storage without creating intermediate extracted files on Mac.
3. **Given** an extraction in progress is canceled by the user, **When** cancellation is triggered, **Then** TTZip halts the USB bulk stream, removes any partially written file, and maintains the remote folder in a consistent state.

---

### User Story 3 - High-Speed Debug Mode and Scoped Storage Penetration (Priority: P3)

As a power user or developer with USB debugging enabled, I want TTZip to automatically switch to the high-speed transfer channel, allowing me to view and manage application-specific data directories (`/Android/data` and `/Android/obb`) at physical USB bus throughput limits.

**Why this priority**:
Modern Android versions (11+) restrict standard file transfer protocols from accessing `/Android/data`. Providing a secondary high-speed channel fulfills the need for complete, unrestricted file system management without rooting the device.

**Independent Test**:
Connect an Android device with USB debugging authorized. Verify TTZip displays a badge indicating "High-Speed Channel Active". Navigate to `/storage/emulated/0/Android/data`: the full list of installed application folders displays and files can be pushed or pulled with zero access denied errors.

**Acceptance Scenarios**:
1. **Given** an Android device with USB debugging active, **When** plugged into Mac, **Then** TTZip automatically negotiates the high-speed debugging protocol without requiring user intervention.
2. **Given** the high-speed channel is established, **When** the user navigates into `/Android/data`, **Then** application package directories are fully visible, readable, and writable.
3. **Given** media files (photos, videos) are written to `/DCIM` or `/Pictures` via the high-speed channel, **When** writing completes, **Then** TTZip triggers an immediate media index update so the phone's native gallery displays the new files instantaneously.

---

### User Story 4 - Cable-Free Wi-Fi Wireless Pairing and Management (Priority: P3)

As a Mac user on the same local Wi-Fi network as my Android device, I want to pair wirelessly using Android's native Wireless Debugging pairing code so that I can inspect and manage files without plugging in a physical USB cable.

**Why this priority**:
Wireless connectivity provides convenience for users when a USB cable is unavailable, utilizing standard mDNS discovery and TLS 1.3 security.

**Independent Test**:
Enable Wireless Debugging on Android 11+. In TTZip, click "Pair Wireless Device" and enter the 6-digit pairing code shown on phone. Verify pairing completes within 5 seconds and the device mounts under Locations with full browsing and transfer capabilities.

**Acceptance Scenarios**:
1. **Given** an Android 11+ device broadcasting `_adb-tls-pairing._tcp` on the local network, **When** the user initiates pairing in TTZip and inputs the 6-digit code, **Then** TTZip authenticates using SPAKE2 over TLS 1.3, caches pairing credentials, and mounts the device.
2. **Given** a paired wireless device rejoins the network, **When** `_adb-tls-connect._tcp` is detected, **Then** TTZip automatically reconnects without re-prompting for the pairing code.

---

### Edge Cases

- **macOS System Daemon Grab**: When macOS `PTPCamera` or third-party image capture services hold the USB interface, the system must seize interface control gracefully without requiring terminal commands or machine reboot.
- **Physical USB Disconnect During Bulk Transfer**: When the USB cable is unplugged while writing a large file, the transfer state machine must terminate pending buffers, release active locks, and emit a user-facing notification rather than hanging the main UI thread.
- **Wi-Fi Signal Drop or IP Shift**: When a wireless device disconnects due to Wi-Fi roaming or sleep state, the system must pause pending transfer tasks, attempt automatic reconnection for up to 10 seconds, and cleanly tear down socket state if unreachable without hanging the UI.
- **USB Pipe Stall**: When the Android hardware FIFO overruns and stalls the endpoint, the system must trigger a multi-stage pipe recovery (abort pending transfers, clear stall flags, reset data toggle) before attempting re-enumeration.
- **Storage Full on Device**: When copying a file larger than the available Android storage capacity, the system must pre-flight check available space and halt before sending data, reporting an explicit "Insufficient Device Storage" warning.
- **Long Path Names and Special Characters**: Filenames containing non-ASCII characters, emojis, or reserved characters must be sanitized and matched according to the target Android filesystem rules.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST automatically detect Android device connection and disconnection events on USB ports without polling loops.
- **FR-002**: System MUST resolve USB interface lock conflicts with macOS system image capture daemons in user-space without kernel extensions or root privileges.
- **FR-003**: System MUST expose connected Android devices as browsable location nodes in the primary application sidebar.
- **FR-004**: System MUST render remote Android directories using multi-column Miller columns view with asynchronous pagination for directories containing over 1,000 items.
- **FR-005**: System MUST provide partial ranged byte-read operations on remote archive files to support reading metadata and central directory structures without downloading the complete file.
- **FR-006**: System MUST stream decompressed data blocks directly into the remote device communication pipe during archive extraction, enforcing zero temporary disk usage on the host Mac.
- **FR-007**: System MUST detect whether the connected device has authorized debugging capabilities and dynamically select the optimal transfer protocol (high-speed debugging channel vs standard file transfer channel).
- **FR-008**: System MUST allow reading and writing files inside external application data directories (`/Android/data`) when operating under the authorized debugging channel.
- **FR-009**: System MUST trigger media index synchronization on the Android device following file writes to public media directories (DCIM, Pictures, Movies, Music), ensuring immediate visibility in native Android applications.
- **FR-010**: System MUST provide cooperative task cancellation for all ongoing file transfers and directory scans, halting remote I/O within 500 milliseconds of user request.
- **FR-011**: System MUST enforce a memory ceiling where remote file browsing and streaming transfers do not exceed 64MB of resident host RAM per active device session.
- **FR-012**: System MUST support local Wi-Fi device discovery via mDNS (`_adb-tls-connect._tcp` and `_adb-tls-pairing._tcp`) and secure authenticated pairing using TLS 1.3 with SPAKE2 for Android 11+ devices.
- **FR-013**: System MUST display a restricted badge on system-isolated directories (`/Android/data` and `/Android/obb`) when operating in standard MTP mode, and present an inline guidance popover directing the user to enable USB debugging to unlock full read/write access.

### Key Entities

- **Device Endpoint**: Represents a physical Android device detected via USB, capturing vendor ID, product ID, serial number, authorized protocol capabilities, and connection status.
- **Storage Partition**: Represents a logical storage volume on the device (e.g., Internal Primary Storage, Removable SD Card), holding capacity, available free space, and mount path.
- **Virtual Directory Node**: Represents a file or folder in the remote Android hierarchy, holding path, display name, byte size, modification timestamp, entry type, and protocol-specific object handle.
- **Transfer Session**: Represents an active read, write, or direct-pipeline extraction job, tracking transfer progress, byte rate, cancellation tokens, and error recovery states.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Connected Android devices appear in the sidebar within 2 seconds of USB attachment.
- **SC-002**: Remote directory navigation renders the first page of items in under 500 milliseconds for folders containing up to 10,000 files under the high-speed channel, and under 2 seconds for standard channel.
- **SC-003**: Remote 4GB ZIP archives open for virtual directory tree exploration in under 100 milliseconds with 0 bytes of local host disk write.
- **SC-004**: Single large file transfer reaches at least 35 MB/s over USB 2.0 physical links and at least 80 MB/s over USB 3.0 links.
- **SC-005**: 100% of photos and videos transferred to the Android device appear in the Android native Gallery within 1 second of transfer completion.
- **SC-006**: Host memory footprint remains below 64MB resident memory during continuous transfer of files exceeding 10GB.
- **SC-007**: Unexpected cable disconnection during active transfer recovers host application state within 1 second without application hang or memory leaks.
- **SC-008**: Wireless pairing completes within 5 seconds of entering the 6-digit PIN on a standard 5GHz Wi-Fi local network.

---

## Assumptions

- **Host Environment**: macOS 13.0 (Ventura) or later running on Apple Silicon or Intel architectures.
- **Device Support**: Android 8.0 through Android 15.0 with standard USB-C or Micro-USB data cables supporting USB 2.0 or 3.0 data lines (not charge-only cables).
- **Zero Companion App Requirement**: The primary user journey must not require installing any third-party APK on the Android device.
- **No System Kernel Extension**: All USB handling must operate within user-space, maintaining compatibility with Apple App Notarization and System Integrity Protection (SIP).
- **Direct VFS Integration**: Android storage will be presented natively within TTZip's UI rather than mounted as a macOS Finder volume to prevent Spotlight indexing storms and QuickLook cache thrashing.
