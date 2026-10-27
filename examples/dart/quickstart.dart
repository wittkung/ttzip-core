// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for Dart & Flutter.
// Standalone runnable quickstart example.

import 'dart:io';
import 'dart:typed_data';
import 'package:ttzip/ttzip.dart';

Future<void> main() async {
  print('⚡️ TTZip Dart & Flutter SDK Quickstart (v${TTZip.version})');
  print('Hardware Accelerated: ${TTZip.isHardwareAccelerated}');

  // 1. Hardware SIMD Checksums
  final payload = Uint8List.fromList(
      'TTZip Dart & Flutter High-Performance Archiving Pipeline 2026'.codeUnits);
  final crc32Val = TTZip.crc32(payload);
  final crc64Val = TTZip.crc64(payload);
  print('SIMD CRC-32: 0x${crc32Val.toRadixString(16).toUpperCase().padLeft(8, '0')}');
  print('SIMD CRC-64: 0x${crc64Val.toRadixString(16).toUpperCase().padLeft(16, '0')}');

  // 2. Setup temporary demo workspace
  final tempDir = await Directory.systemTemp.createTemp('ttzip_dart_quickstart_');
  try {
    final file1 = File('${tempDir.path}/app_config.json');
    final file2 = File('${tempDir.path}/user_data.txt');

    await file1.writeAsString('{"framework": "Flutter", "engine": "TTZip", "year": 2026}');
    await file2.writeAsString('High-throughput background Isolate archive compression payload.');

    final zipPath = '${tempDir.path}/quickstart_demo.zip';
    final extractDir = '${tempDir.path}/extracted';
    await Directory(extractDir).create(recursive: true);

    // 3. Compress using background Isolate
    print('\nCreating archive with background Isolate worker...');
    await TTZip.compress(
      sources: [file1.path, file2.path],
      destination: zipPath,
      format: TTZipFormat.zip,
      level: TTZipCompressionLevel.normal,
      threads: 2,
    );
    print('Created archive: $zipPath (size: ${await File(zipPath).length()} bytes)');

    // 4. Stream real-time compression progress
    print('\nDemonstrating real-time Stream<ArchiveProgress> compression:');
    final streamOut = '${tempDir.path}/stream_demo.zip';
    final progressStream = TTZip.compressStream(
      sources: [file1.path, file2.path],
      destination: streamOut,
      level: TTZipCompressionLevel.fastest,
    );

    await for (final progress in progressStream) {
      print('  Progress: ${(progress.fractionCompleted * 100).toStringAsFixed(1)}% '
          '(${progress.processedBytes}/${progress.totalBytes} bytes) - phase: ${progress.phase}');
    }

    // 5. Extract archive
    print('\nExtracting archive...');
    await TTZip.extract(
      archivePath: zipPath,
      destination: extractDir,
      threads: 2,
    );
    print('Successfully extracted to: $extractDir');

    final extractedFile = File('$extractDir/app_config.json');
    if (await extractedFile.exists()) {
      print('Verified extracted payload: ${await extractedFile.readAsString()}');
    }

    print('\n✅ TTZip Dart & Flutter Quickstart completed successfully.');
  } finally {
    if (await tempDir.exists()) {
      await tempDir.delete(recursive: true);
    }
  }
}
