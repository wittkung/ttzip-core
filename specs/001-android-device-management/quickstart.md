# Quickstart & End-to-End Validation Guide: Android Device Management

**Feature Branch**: `001-android-device-management`
**Date**: 2026-09-13
**Status**: Ready for Implementation

---

## 1. Prerequisites & Environment Setup

### 1.1 Host Environment
- macOS 13.0+ (Ventura, Sonoma, Sequoia) running on Apple Silicon (arm64) or Intel (x86_64).
- Xcode 15+ command line tools installed.
- Rust toolchain 1.75+ with `cargo`.

### 1.2 Target Android Device
- Android device running Android 8.0 through Android 15.0.
- Standard USB-C to USB-C / USB-A data cable (non-charging-only).
- Developer Options & USB Debugging (optional, required only for High-Speed & Scoped Storage penetration validation).

---

## 2. End-to-End Validation Scenarios

### Scenario 1: USB Detection & Non-Conflict Interface Seize (Plug-and-Play)
**Goal**: Verify that TTZip discovers the USB device, seizes the interface from macOS `PTPCamera.app`, and lists root partitions without error.

**Execution Steps**:
1. Connect Android phone via USB and select "File Transfer / MTP" on the phone lock screen.
2. Launch TTZip debug build or run the hardware harness:
   ```bash
   cargo test -p ttzip-device-android --test usb_claim_tests -- --nocapture
   ```
3. Observe test output:
   - Device detection event emits VendorID and ProductID.
   - IOKit `USBInterfaceOpenSeize` returns `kIOReturnSuccess (0)`.
   - Device status transitions: `Connecting` -> `SeizingInterface` -> `Connected`.
   - Root partitions (`Internal Storage`) emit total and available byte counts.

**Expected Outcome**:
Zero `kIOReturnExclusiveAccess` (0xE00002C5) errors. Sidebar dynamically presents the device under the "Locations" category.

---

### Scenario 2: Zero-Download 4GB Archive Instant Tree Inspection
**Goal**: Validate the Stream-First Invariant by inspecting a 4GB+ remote ZIP archive without downloading it to local disk.

**Execution Steps**:
1. Ensure a 4GB+ `.zip` file exists on the Android device under `/Download/large_archive.zip`.
2. Run the archive inspection harness:
   ```bash
   cargo test -p ttzip-device-android --test stream_inspection_tests -- --nocapture
   ```
3. Monitor network/bus transfer bytes and local host disk writes via system profiler:
   - Bus traffic must not exceed 512KB (reading only EOCD and Central Directory headers via `GetPartialObject64`).
   - Mac local disk write must strictly register `0 bytes`.
   - Wall-clock time to build full `VfsTree` must be $<50\text{ms}$.

**Expected Outcome**:
Full directory tree renders in TTZip Miller columns view instantly. Host RAM remains $\le 4\text{MB}$.

---

### Scenario 3: Direct Pipeline Decompression (Zero Host Disk Temp File)
**Goal**: Decompress a Mac local ZIP archive directly into an Android device directory without intermediate extraction to Mac `/tmp`.

**Execution Steps**:
1. Prepare a 1GB test ZIP file on Mac: `/tmp/test_dataset.zip`.
2. Execute direct pipeline extraction to remote destination `/Download/Extracted/`:
   ```bash
   swift test --filter AndroidExtractionTests
   ```
3. Audit Mac disk space during decompression:
   - Check available disk space on Mac: `df -h /` should experience 0 byte reduction.
   - Inspect Android storage: files appear in `/storage/emulated/0/Download/Extracted/` matching CRC32 checksums.

**Expected Outcome**:
Data stream flows directly: `archive_read_data_block` -> memory buffer -> USB Bulk OUT endpoint -> Android flash storage. Zero staging files created on macOS.

---

### Scenario 4: High-Speed Mode & `/Android/data` Penetration
**Goal**: Verify ADB channel activation, high throughput, and penetration into restricted application data directories on Android 11+.

**Execution Steps**:
1. Authorize USB Debugging on the Android phone.
2. Execute ADB channel integration test:
   ```bash
   cargo test -p ttzip-device-android --test adb_sync_tests -- --nocapture
   ```
3. Verify directory listing for `/storage/emulated/0/Android/data`:
   - System displays installed application package directories without `EACCES` or permission denied errors.
   - Push a test file `probe.txt` into `/Android/data/com.example.test/`: write succeeds.
4. Execute media scan update for a written photo:
   - Push `sample.jpg` to `/sdcard/DCIM/Camera/`.
   - Trigger `content call --uri content://media/ --method scan_file --arg /storage/emulated/0/DCIM/Camera/sample.jpg`.
   - Verify Android system Gallery displays `sample.jpg` within 1 second.

**Expected Outcome**:
Throughput reaches $>35\text{MB/s}$ over USB 2.0. Protected directories fully accessible without root.

---

### Scenario 5: Wi-Fi Wireless Pairing with 6-Digit PIN (Android 11+)
**Goal**: Verify mDNS discovery and TLS 1.3 SPAKE2 wireless pairing over local Wi-Fi.

**Execution Steps**:
1. Connect Mac and Android phone to the same 5GHz Wi-Fi access point.
2. On Android phone: Settings -> Developer Options -> Wireless Debugging -> "Pair device with pairing code".
3. Run the wireless pairing test suite with pairing arguments:
   ```bash
   cargo test -p ttzip-device-android --test wireless_pairing_tests -- --nocapture
   ```
4. Verify:
   - mDNS resolves `_adb-tls-pairing._tcp` within 2 seconds.
   - SPAKE2 key exchange over TLS 1.3 completes in $<1$ second.
   - Subsequent connection to `_adb-tls-connect._tcp` mounts device storage without USB cable.

**Expected Outcome**:
Device displays in TTZip sidebar with Wi-Fi indicator icon, matching physical USB capabilities.
