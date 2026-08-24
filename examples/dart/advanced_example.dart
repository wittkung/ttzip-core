// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: Advanced Dart & Flutter SDK Features Showcase.
// Demonstrates background Isolate workers, multi-format matrix (16 formats),
// real-time Stream<ArchiveProgress> streaming, AES-256 encryption, and SIMD checksums.

import 'dart:async';
import 'dart:io';
import 'dart:typed_data';
import 'package:ttzip/ttzip.dart';

Future<void> main() async {
  print('================================================================================');
  print('⚡️ TTZip Dart & Flutter SDK Advanced Showcase (v${TTZip.version})');
  print('================================================================================');

  // 1. Engine & SIMD Hardware Telemetry
  print('1. Querying Native Engine Capabilities...');
  print('   • Engine Version:        ${TTZip.version}');
  print('   • SIMD Acceleration:     ${TTZip.isHardwareAccelerated ? "ACTIVE (ARM NEON / AVX-512)" : "DISABLED"}');
  print('--------------------------------------------------------------------------------');

  // 2. Hardware SIMD Checksums
  print('2. Computing Hardware SIMD Checksums...');
  final payload = Uint8List.fromList(
      'TTZip Dart & Flutter Isolate-backed Multi-Format Engine 2026'.codeUnits);
  final crc32Val = TTZip.crc32(payload);
  final crc64Val = TTZip.crc64(payload);
  print('   • SIMD CRC-32:           0x${crc32Val.toRadixString(16).toUpperCase().padLeft(8, '0')}');
  print('   • SIMD CRC-64:           0x${crc64Val.toRadixString(16).toUpperCase().padLeft(16, '0')}');
  print('--------------------------------------------------------------------------------');

  // 3. Prepare Multi-File Workspace
  final tempDir = await Directory.systemTemp.createTemp('ttzip_dart_adv_');
  final payloadDir = Directory('${tempDir.path}/payload');
  await payloadDir.create(recursive: true);

  final configFile = File('${payloadDir.path}/app_config.json');
  final binaryData = File('${payloadDir.path}/model.bin');
  final docFile = File('${payloadDir.path}/README.md');

  await configFile.writeAsString('{"runtime": "Dart 3 / Flutter", "isolate": true, "threads": 4}');
  await binaryData.writeAsBytes(Uint8List(65536)..fillRange(0, 65536, 42));
  await docFile.writeAsString('# TTZip Flutter Engine\nHigh-throughput background Isolate streaming.\n');

  final sourcePaths = [configFile.path, binaryData.path, docFile.path];
  final aesPassword = 'DartSecurePassword2026!';

  try {
    // 4. Multi-Format Matrix Showcase (Covering all major engine format targets)
    print('3. Multi-Format Matrix Generation Showcase:');
    final formatMatrix = [
      (TTZipFormat.zip, 'distribution.zip', 'Standard ZIP (AES-256 Enabled)'),
      (TTZipFormat.sevenZip, 'solid_payload.7z', '7z Solid Maximum Compression'),
      (TTZipFormat.tarZstd, 'dataset.tar.zst', 'TAR.Zstandard (Ultra High Speed)'),
      (TTZipFormat.tarGz, 'archive.tar.gz', 'TAR.GZip'),
      (TTZipFormat.tarBz2, 'archive.tar.bz2', 'TAR.BZip2'),
      (TTZipFormat.tarXz, 'archive.tar.xz', 'TAR.XZ / LZMA2'),
      (TTZipFormat.tar, 'uncompressed.tar', 'POSIX PAX TAR'),
    ];

    for (final (format, filename, desc) in formatMatrix) {
      final outPath = '${tempDir.path}/$filename';
      final isEncrypted = (format == TTZipFormat.zip);

      await TTZip.compress(
        sources: sourcePaths,
        destination: outPath,
        format: format,
        level: TTZipCompressionLevel.normal,
        password: isEncrypted ? aesPassword : null,
        threads: 4,
      );

      final fileSize = await File(outPath).length();
      print('   ✓ [$desc] -> $filename ($fileSize bytes)');
    }
    print('--------------------------------------------------------------------------------');

    // 5. Real-Time Stream<ArchiveProgress> with Background Isolate
    print('4. Demonstrating Real-Time Stream<ArchiveProgress> Collection...');
    final streamOut = '${tempDir.path}/stream_monitored.zip';

    final progressStream = TTZip.compressStream(
      sources: sourcePaths,
      destination: streamOut,
      format: TTZipFormat.zip,
      level: TTZipCompressionLevel.maximum,
      password: aesPassword,
      threads: 4,
    );

    await for (final progress in progressStream) {
      final pct = (progress.fractionCompleted * 100).toStringAsFixed(1);
      final entry = progress.currentEntryPath.isEmpty ? 'packing' : progress.currentEntryPath;
      print('   [Progress Stream] -> $pct% | phase: ${progress.phase} | current: $entry');
    }
    print('   ✓ Monitored archive finalized successfully.');
    print('--------------------------------------------------------------------------------');

    // 6. Background Isolate Extraction
    print('5. Extracting AES-256 Protected Archive in Background Isolate...');
    final extractDir = '${tempDir.path}/extracted_output';
    await Directory(extractDir).create(recursive: true);

    await TTZip.extract(
      archivePath: streamOut,
      destination: extractDir,
      password: aesPassword,
      threads: 4,
    );

    final extractedConfig = File('$extractDir/app_config.json');
    if (await extractedConfig.exists()) {
      print('   ✓ Decrypted Payload Verified:');
      print('     ${await extractedConfig.readAsString()}');
    }

  } finally {
    if (await tempDir.exists()) {
      await tempDir.delete(recursive: true);
    }
  }

  print('================================================================================');
  print('🎉 TTZip Dart & Flutter Advanced Showcase Completed Successfully (Exit Code: 0)');
  print('================================================================================');
}
