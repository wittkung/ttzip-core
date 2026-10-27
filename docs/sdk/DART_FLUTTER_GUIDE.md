# 🎯 TTZip Dart & Flutter Developer Guide

[![pub package](https://img.shields.io/badge/pub.dev-ttzip%20v1.0.0-blue.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/sdk/dart/lib/ttzip.dart)
[![Platforms](https://img.shields.io/badge/Platforms-Flutter%20%7C%20Dart%20VM%20%7C%20macOS%20%7C%20iOS%20%7C%20Android%20%7C%20Windows%20%7C%20Linux-brightgreen.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/sdk/dart/pubspec.yaml)
[![Concurrency: Isolates](https://img.shields.io/badge/Threading-Background%20Isolates%20(Zero%20UI%20Stutter)-purple.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/sdk/dart/lib/ttzip.dart#L234)

The `ttzip` Dart & Flutter package provides high-performance native archiving and decompression for mobile, desktop, and server applications. Using **`dart:ffi` with background `Isolate.run` worker pools**, it offloads heavy cryptographic and compression tasks off the Flutter UI thread to maintain 120 FPS frame rates.

---

## 1. Installation

Add `ttzip` to your `pubspec.yaml`:

```yaml
dependencies:
  flutter:
    sdk: flutter
  ttzip: ^1.0.0
  ffi: ^2.1.2
```

### Dynamic Library Resolution

`ttzip` automatically detects the host platform and loads the compiled native binary:
- **macOS / iOS**: `TTZipVendor.framework` / `libttzip_engine.dylib`
- **Android**: `libttzip_engine.so` in `android/app/src/main/jniLibs/<abi>`
- **Windows**: `ttzip_engine.dll` in executable directory
- **Linux**: `libttzip_engine.so` in system library path or app bundle

---

## 2. Quickstart Code Examples

### 2.1 Non-Blocking Archive Compression

Compress files asynchronously inside a background isolate:

```dart
import 'package:flutter/foundation.dart';
import 'package:ttzip/ttzip.dart';

Future<void> backupUserData() async {
  final sourceFiles = [
    '/data/user/0/com.example.app/files/documents',
    '/data/user/0/com.example.app/files/database.sqlite',
  ];
  final targetArchive = '/data/user/0/com.example.app/files/backup.7z';

  try {
    await TTZip.compress(
      sources: sourceFiles,
      destination: targetArchive,
      format: TTZipFormat.sevenZip,
      level: TTZipCompressionLevel.normal, // Level 6
      password: 'UserSecretPassword123!',
      threads: 0, // Auto-detect CPU cores
    );
    debugPrint('Backup successfully created at: $targetArchive');
  } catch (e) {
    debugPrint('Compression failed: $e');
  }
}
```

### 2.2 Safe Archive Extraction (Zip-Slip Immune)

```dart
import 'package:flutter/foundation.dart';
import 'package:ttzip/ttzip.dart';

Future<void> unpackArchive(String archivePath, String outputDirectory) async {
  try {
    await TTZip.extract(
      archivePath: archivePath,
      destination: outputDirectory,
      password: 'UserSecretPassword123!',
    );
    debugPrint('All files unpacked safely to: $outputDirectory');
  } catch (e) {
    debugPrint('Extraction error: $e');
  }
}
```

---

## 3. Flutter UI Progress Streaming (`StreamBuilder`)

Stream live progress events directly into your Flutter UI:

```dart
import 'package:flutter/material.dart';
import 'package:ttzip/ttzip.dart';

class ArchiveProgressScreen extends StatelessWidget {
  final String archivePath;
  final String destination;

  const ArchiveProgressScreen({
    super.key,
    required this.archivePath,
    required this.destination,
  });

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Extracting Archive')),
      body: StreamBuilder<ArchiveProgress>(
        stream: TTZip.extractStream(
          archivePath: archivePath,
          destination: destination,
        ),
        builder: (context, snapshot) {
          if (snapshot.hasError) {
            return Center(child: Text('Error: ${snapshot.error}'));
          }

          final progress = snapshot.data;
          final fraction = progress?.fractionCompleted ?? 0.0;
          final currentFile = progress?.currentEntryPath ?? 'Preparing...';

          return Padding(
            padding: const EdgeInsets.all(24.0),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                LinearProgressIndicator(value: fraction),
                const SizedBox(height: 16),
                Text('${(fraction * 100).toStringAsFixed(1)}% Completed'),
                const SizedBox(height: 8),
                Text(
                  'Extracting: $currentFile',
                  style: Theme.of(context).textTheme.bodySmall,
                  overflow: TextOverflow.ellipsis,
                ),
              ],
            ),
          );
        },
      ),
    );
  }
}
```

---

## 4. Hardware SIMD Checksums on `Uint8List`

Calculate hardware-accelerated CRC-32 (>40 GB/s on Apple Silicon / AVX-512) and CRC-64 directly on Dart `Uint8List` buffers:

```dart
import 'dart:typed_data';
import 'package:ttzip/ttzip.dart';

void computeChecksums() {
  final payload = Uint8List.fromList('TTZip Flutter High-Speed Vectorized Payload'.codeUnits);

  final crc32Value = TTZip.crc32(payload);
  final crc64Value = TTZip.crc64(payload);

  print('CRC-32: 0x${crc32Value.toRadixString(16).toUpperCase().padLeft(8, '0')}');
  print('CRC-64: 0x${crc64Value.toRadixString(16).toUpperCase().padLeft(16, '0')}');
  print('Hardware Accelerated: ${TTZip.isHardwareAccelerated}');
}
```

---

## 5. Mobile & Desktop Concurrency Guidelines

1. **Zero UI Thread Blocking**: All compression and extraction functions execute within ephemeral background Dart isolates via `Isolate.run`.
2. **Memory Efficiency**: Native buffers are allocated inside scoped `using((arena) { ... })` blocks and immediately freed after FFI calls return.
3. **App Sandbox Compliance**: Full support for iOS and macOS sandboxed application containers and security-scoped URLs.
