// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for Dart & Flutter.
// dart test suite validating dart:ffi bindings, background Isolate compute jobs, and Stream emissions.

import 'dart:async';
import 'dart:io';
import 'dart:typed_data';
import 'package:test/test.dart';
import '../lib/ttzip.dart';

void main() {
  group('TTZip Dart & Flutter SDK Test Suite', () {
    late Directory tempDir;

    setUp(() async {
      tempDir = await Directory.systemTemp.createTemp('ttzip_dart_test_');
    });

    tearDown(() async {
      if (await tempDir.exists()) {
        await tempDir.delete(recursive: true);
      }
    });

    test('validates dart:ffi bindings and hardware acceleration detection', () {
      expect(TTZip.version, isNotEmpty);
      expect(TTZip.version, equals('1.0.0'));

      final isHw = TTZip.isHardwareAccelerated;
      expect(isHw, isA<bool>());
    });

    test('validates SIMD hardware-accelerated CRC-32 calculation', () {
      final payload = Uint8List.fromList(
          'TTZip Dart High-Performance SIMD CRC-32 Acceleration 2026'
              .codeUnits);

      final crc = TTZip.crc32(payload);
      expect(crc, isNonZero);

      // Verify incremental / seeded CRC-32 computation
      final half = payload.length ~/ 2;
      final firstHalf = Uint8List.sublistView(payload, 0, half);
      final secondHalf = Uint8List.sublistView(payload, half);

      final seed = TTZip.crc32(firstHalf, 0);
      final chained = TTZip.crc32(secondHalf, seed);
      expect(chained, equals(crc));
    });

    test('validates SIMD hardware-accelerated CRC-64 calculation', () {
      final payload = Uint8List.fromList(
          'TTZip Dart High-Performance SIMD CRC-64 Verification'
              .codeUnits);

      final crc64Val = TTZip.crc64(payload);
      expect(crc64Val, isNonZero);
    });

    test('validates background Isolate compute jobs for compression and extraction', () async {
      final sampleFile = File('${tempDir.path}/sample.txt');
      const sampleText = 'Dart background Isolate archiving execution with TTZip';
      await sampleFile.writeAsString(sampleText);

      final archivePath = '${tempDir.path}/isolate_archive.zip';
      final extractDir = '${tempDir.path}/isolate_extracted';
      await Directory(extractDir).create(recursive: true);

      // Background Isolate Compression
      await TTZip.compress(
        sources: [sampleFile.path],
        destination: archivePath,
        format: TTZipFormat.zip,
        level: TTZipCompressionLevel.normal,
        threads: 2,
      );

      final archiveFile = File(archivePath);
      expect(await archiveFile.exists(), isTrue);
      expect(await archiveFile.length(), greaterThan(0));

      // Background Isolate Extraction
      await TTZip.extract(
        archivePath: archivePath,
        destination: extractDir,
        threads: 2,
      );

      final extractedFile = File('$extractDir/sample.txt');
      expect(await extractedFile.exists(), isTrue);
      final readBack = await extractedFile.readAsString();
      expect(readBack, equals(sampleText));
    });

    test('validates Stream<ArchiveProgress> emissions during compression', () async {
      final file1 = File('${tempDir.path}/stream_doc1.txt');
      final file2 = File('${tempDir.path}/stream_doc2.txt');
      await file1.writeAsString('Stream document 1 payload data ' * 200);
      await file2.writeAsString('Stream document 2 payload data ' * 200);

      final archivePath = '${tempDir.path}/stream_output.zip';

      final progressEvents = <ArchiveProgress>[];
      final completer = Completer<void>();

      final stream = TTZip.compressStream(
        sources: [file1.path, file2.path],
        destination: archivePath,
        format: TTZipFormat.zip,
        level: TTZipCompressionLevel.fastest,
        threads: 1,
      );

      stream.listen(
        (progress) {
          progressEvents.add(progress);
        },
        onError: (error) {
          completer.completeError(error);
        },
        onDone: () {
          completer.complete();
        },
      );

      await completer.future;

      expect(progressEvents, isNotEmpty);
      expect(await File(archivePath).exists(), isTrue);
      expect(progressEvents.last.fractionCompleted, equals(1.0));
      expect(progressEvents.last.phase, equals('completed'));
    });

    test('validates Stream<ArchiveProgress> emissions during extraction', () async {
      final sampleFile = File('${tempDir.path}/extract_stream.txt');
      await sampleFile.writeAsString('Streaming extraction verification test');

      final archivePath = '${tempDir.path}/extract_stream.zip';
      final extractDir = '${tempDir.path}/extract_stream_out';
      await Directory(extractDir).create(recursive: true);

      await TTZip.compress(
        sources: [sampleFile.path],
        destination: archivePath,
        format: TTZipFormat.zip,
      );

      final progressEvents = <ArchiveProgress>[];
      final completer = Completer<void>();

      final stream = TTZip.extractStream(
        archivePath: archivePath,
        destination: extractDir,
      );

      stream.listen(
        (progress) {
          progressEvents.add(progress);
        },
        onError: (error) {
          completer.completeError(error);
        },
        onDone: () {
          completer.complete();
        },
      );

      await completer.future;

      expect(progressEvents, isNotEmpty);
      expect(await File('$extractDir/extract_stream.txt').exists(), isTrue);
      expect(progressEvents.last.fractionCompleted, equals(1.0));
    });

    test('validates multi-file archive creation across formats', () async {
      final dirA = Directory('${tempDir.path}/folderA');
      await dirA.create(recursive: true);
      final item1 = File('${dirA.path}/item1.txt');
      final item2 = File('${dirA.path}/item2.log');
      await item1.writeAsString('Item 1 Content');
      await item2.writeAsString('Item 2 Log Message');

      final formats = [
        (TTZipFormat.zip, 'multi.zip', TTZipCompressionLevel.fastest),
        (TTZipFormat.sevenZip, 'multi.7z', TTZipCompressionLevel.normal),
        (TTZipFormat.tar, 'multi.tar', TTZipCompressionLevel.store),
        (TTZipFormat.tarGz, 'multi.tar.gz', TTZipCompressionLevel.fast),
        (TTZipFormat.tarBz2, 'multi.tar.bz2', TTZipCompressionLevel.normal),
        (TTZipFormat.tarXz, 'multi.tar.xz', TTZipCompressionLevel.maximum),
        (TTZipFormat.tarZstd, 'multi.tar.zst', TTZipCompressionLevel.ultra),
      ];

      for (final (fmt, filename, lvl) in formats) {
        final outPath = '${tempDir.path}/$filename';
        final destPath = '${tempDir.path}/dest_${fmt.name}';
        await Directory(destPath).create(recursive: true);

        await TTZip.compress(
          sources: [dirA.path],
          destination: outPath,
          format: fmt,
          level: lvl,
        );

        expect(await File(outPath).exists(), isTrue);
        expect(await File(outPath).length(), greaterThan(0));

        await TTZip.extract(
          archivePath: outPath,
          destination: destPath,
        );

        final read1 = File('$destPath/folderA/item1.txt');
        if (await read1.exists()) {
          expect(await read1.readAsString(), equals('Item 1 Content'));
        }
      }
    });

    test('validates AES-256 password-protected archive creation and extraction', () async {
      final secretFile = File('${tempDir.path}/dart_secret.txt');
      const secretPayload = 'Dart FFI AES-256 Protected Archive Secret Payload 2026';
      await secretFile.writeAsString(secretPayload);

      final encPath = '${tempDir.path}/dart_encrypted.zip';
      final validDest = '${tempDir.path}/dart_decrypted_valid';
      final invalidDest = '${tempDir.path}/dart_decrypted_invalid';
      await Directory(validDest).create(recursive: true);
      await Directory(invalidDest).create(recursive: true);

      const correctPassword = 'DartSecretPassword2026!';
      const wrongPassword = 'WrongDartPassword999!';

      // 1. Compress with password
      await TTZip.compress(
        sources: [secretFile.path],
        destination: encPath,
        format: TTZipFormat.zip,
        level: TTZipCompressionLevel.normal,
        password: correctPassword,
      );

      expect(await File(encPath).exists(), isTrue);

      // 2. Extract with correct password
      await TTZip.extract(
        archivePath: encPath,
        destination: validDest,
        password: correctPassword,
      );

      final decrypted = File('$validDest/dart_secret.txt');
      expect(await decrypted.exists(), isTrue);
      expect(await decrypted.readAsString(), equals(secretPayload));

      // 3. Extract with wrong password -> must throw
      expect(
        () async => await TTZip.extract(
          archivePath: encPath,
          destination: invalidDest,
          password: wrongPassword,
        ),
        throwsA(isA<Exception>()),
      );
    });

    test('validates corrupted archive header detection', () async {
      final corruptFile = File('${tempDir.path}/dart_corrupt.zip');
      await corruptFile.writeAsBytes([0x50, 0x4B, 0x03, 0x04, 0x00, 0x00, 0xFF, 0xFF, 0x11, 0x22]);

      final corruptOut = '${tempDir.path}/dart_corrupt_out';
      await Directory(corruptOut).create(recursive: true);

      expect(
        () async => await TTZip.extract(
          archivePath: corruptFile.path,
          destination: corruptOut,
        ),
        throwsA(isA<Exception>()),
      );
    });
  });
}
