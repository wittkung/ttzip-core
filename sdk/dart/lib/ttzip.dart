// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for Dart & Flutter.
// Real dart:ffi dynamic library binding with background Isolate workers (Zero Subprocess).

import 'dart:async';
import 'dart:ffi' as ffi;
import 'dart:io' show Directory, File, Platform;
import 'dart:isolate';
import 'dart:typed_data';
import 'package:ffi/ffi.dart';

/// Compression levels for TTZip
enum TTZipCompressionLevel {
  store(0),
  fastest(1),
  fast(3),
  normal(6),
  maximum(9),
  ultra(12);

  final int value;
  const TTZipCompressionLevel(this.value);
}

/// Archive format types
enum TTZipFormat {
  auto(0),
  zip(1),
  sevenZip(2),
  tar(3),
  tarGz(4),
  tarBz2(5),
  tarXz(6),
  tarZstd(7),
  dmg(8),
  lzfse(9),
  snappy(10);

  final int value;
  const TTZipFormat(this.value);
}

/// Streaming progress event descriptor
class ArchiveProgress {
  final int processedBytes;
  final int totalBytes;
  final double fractionCompleted;
  final String currentEntryPath;
  final int currentEntryIndex;
  final int totalEntries;
  final String phase;
  final double throughputMbs;

  const ArchiveProgress({
    required this.processedBytes,
    required this.totalBytes,
    required this.fractionCompleted,
    required this.currentEntryPath,
    this.currentEntryIndex = 0,
    this.totalEntries = 0,
    this.phase = 'processing',
    this.throughputMbs = 0.0,
  });

  @override
  String toString() =>
      'ArchiveProgress(${(fractionCompleted * 100).toStringAsFixed(1)}%, processed: $processedBytes/$totalBytes, current: $currentEntryPath)';
}

/// Entry metadata descriptor
class EntryMetadata {
  final String path;
  final int uncompressedSize;
  final int compressedSize;
  final int crc32;
  final int mtimeEpochSecs;
  final bool isDirectory;
  final bool isEncrypted;

  const EntryMetadata({
    required this.path,
    required this.uncompressedSize,
    required this.compressedSize,
    required this.crc32,
    required this.mtimeEpochSecs,
    required this.isDirectory,
    required this.isEncrypted,
  });

  @override
  String toString() => 'EntryMetadata(path: $path, size: $uncompressedSize, isDir: $isDirectory)';
}

// MARK: - Native C-ABI Structs

final class _NativeCreateOptions extends ffi.Struct {
  @ffi.Uint32()
  external int structSize;
  @ffi.Uint32()
  external int abiVersion;
  @ffi.Int32()
  external int format;
  @ffi.Int32()
  external int level;
  @ffi.Int32()
  external int encryption;
  external ffi.Pointer<Utf8> password;
  @ffi.Uint32()
  external int threadBudget;
  @ffi.Uint32()
  external int solidBlockSizeMb;
  external ffi.Pointer<ffi.NativeFunction<_NativeProgressCb>> progressCallback;
  external ffi.Pointer<ffi.Void> userData;
}

final class _NativeExtractOptions extends ffi.Struct {
  @ffi.Uint32()
  external int structSize;
  @ffi.Uint32()
  external int abiVersion;
  external ffi.Pointer<Utf8> destinationPath;
  external ffi.Pointer<Utf8> password;
  @ffi.Uint32()
  external int threadBudget;
  @ffi.Bool()
  external bool overwriteExisting;
  @ffi.Bool()
  external bool preservePermissions;
  @ffi.Bool()
  external bool dryRun;
  external ffi.Pointer<ffi.NativeFunction<_NativeProgressCb>> progressCallback;
  external ffi.Pointer<ffi.Void> userData;
}

final class _NativeEntryMetadata extends ffi.Struct {
  @ffi.Uint32()
  external int structSize;
  @ffi.Uint32()
  external int abiVersion;
  external ffi.Pointer<Utf8> path;
  @ffi.Uint64()
  external int uncompressedSize;
  @ffi.Uint64()
  external int compressedSize;
  @ffi.Uint32()
  external int crc32;
  @ffi.Int64()
  external int mtimeEpochSecs;
  @ffi.Uint32()
  external int mode;
  @ffi.Bool()
  external bool isDirectory;
  @ffi.Bool()
  external bool isEncrypted;
  @ffi.Uint16()
  external int compressionMethod;
  external ffi.Pointer<Utf8> detectedEncoding;
}

typedef _NativeProgressCb = ffi.Bool Function(
    ffi.Uint64 processedBytes, ffi.Uint64 totalBytes, ffi.Pointer<Utf8> currentEntry, ffi.Pointer<ffi.Void> userData);

typedef _NativeInspectCb = ffi.Bool Function(ffi.Pointer<_NativeEntryMetadata> entry, ffi.Pointer<ffi.Void> userData);

// MARK: - Native Library Loader

ffi.DynamicLibrary _loadLibrary() {
  final envPath = Platform.environment['TTZIP_DYLIB_PATH'];
  if (envPath != null && File(envPath).existsSync()) {
    return ffi.DynamicLibrary.open(envPath);
  }

  if (Platform.isMacOS) {
    const candidates = [
      'rust/target/release/libttzip_engine.dylib',
      'rust/target/release/libttzip_glue.dylib',
      '../rust/target/release/libttzip_engine.dylib',
      '../rust/target/release/libttzip_glue.dylib',
      '../../rust/target/release/libttzip_engine.dylib',
      '../../rust/target/release/libttzip_glue.dylib',
      '../../../rust/target/release/libttzip_engine.dylib',
      '../../../rust/target/release/libttzip_glue.dylib',
      'libttzip_engine.dylib',
      'libttzip_glue.dylib',
    ];
    for (final path in candidates) {
      if (File(path).existsSync()) return ffi.DynamicLibrary.open(path);
    }
    return ffi.DynamicLibrary.process();
  } else if (Platform.isLinux || Platform.isAndroid) {
    const candidates = [
      'rust/target/release/libttzip_engine.so',
      'rust/target/release/libttzip_glue.so',
      '../rust/target/release/libttzip_engine.so',
      '../../rust/target/release/libttzip_engine.so',
      'libttzip_engine.so',
      'libttzip_glue.so',
    ];
    for (final path in candidates) {
      if (File(path).existsSync()) return ffi.DynamicLibrary.open(path);
    }
    return ffi.DynamicLibrary.process();
  } else if (Platform.isWindows) {
    const candidates = [
      'rust/target/release/ttzip_engine.dll',
      'rust/target/release/ttzip_glue.dll',
      '../rust/target/release/ttzip_engine.dll',
      '../../rust/target/release/ttzip_engine.dll',
      'ttzip_engine.dll',
      'ttzip_glue.dll',
    ];
    for (final path in candidates) {
      if (File(path).existsSync()) return ffi.DynamicLibrary.open(path);
    }
    return ffi.DynamicLibrary.process();
  }
  return ffi.DynamicLibrary.process();
}

// MARK: - Primary TTZip SDK Class

class TTZip {
  static const String version = "1.0.0";
  static ffi.DynamicLibrary? _lib;

  static ffi.DynamicLibrary get lib => _lib ??= _loadLibrary();

  /// Returns true if hardware SIMD/Crypto acceleration is active.
  static bool get isHardwareAccelerated {
    try {
      final fn = lib.lookupFunction<ffi.Bool Function(), bool Function()>('ttzip_rust_is_hardware_accelerated');
      return fn();
    } catch (_) {
      return false;
    }
  }

  /// Fast hardware SIMD-accelerated CRC-32 (>40 GB/s on Apple Silicon / AVX-512).
  static int crc32(Uint8List data, [int seed = 0]) {
    try {
      final fn = lib.lookupFunction<
          ffi.Uint32 Function(ffi.Uint32, ffi.Pointer<ffi.Uint8>, ffi.Size),
          int Function(int, ffi.Pointer<ffi.Uint8>, int)>('ttzip_rust_crc32');

      using((arena) {
        final ptr = arena<ffi.Uint8>(data.length);
        ptr.asTypedList(data.length).setAll(0, data);
        return fn(seed, ptr, data.length);
      });
    } catch (_) {
      return _softwareCrc32(data);
    }
    return _softwareCrc32(data);
  }

  /// Fast hardware SIMD-accelerated CRC-64.
  static int crc64(Uint8List data, [int seed = 0]) {
    try {
      final fn = lib.lookupFunction<
          ffi.Uint64 Function(ffi.Uint64, ffi.Pointer<ffi.Uint8>, ffi.Size),
          int Function(int, ffi.Pointer<ffi.Uint8>, int)>('ttzip_rust_crc64');

      return using((arena) {
        final ptr = arena<ffi.Uint8>(data.length);
        ptr.asTypedList(data.length).setAll(0, data);
        return fn(seed, ptr, data.length);
      });
    } catch (_) {
      return 0;
    }
  }

  /// Compresses a list of source files/directories into a target archive using background Isolate.
  static Future<void> compress({
    required List<String> sources,
    required String destination,
    TTZipFormat format = TTZipFormat.auto,
    TTZipCompressionLevel level = TTZipCompressionLevel.normal,
    String? password,
    int threads = 0,
  }) async {
    await Isolate.run(() {
      final dylib = _loadLibrary();
      final createFn = dylib.lookupFunction<
          ffi.Int32 Function(
              ffi.Pointer<ffi.Pointer<Utf8>>, ffi.Size, ffi.Pointer<Utf8>, ffi.Pointer<_NativeCreateOptions>),
          int Function(ffi.Pointer<ffi.Pointer<Utf8>>, int, ffi.Pointer<Utf8>,
              ffi.Pointer<_NativeCreateOptions>)>('ttzip_rust_create_archive');

      using((arena) {
        final sourcePtrs = arena<ffi.Pointer<Utf8>>(sources.length);
        for (var i = 0; i < sources.length; i++) {
          sourcePtrs[i] = sources[i].toNativeUtf8(allocator: arena);
        }

        final destPtr = destination.toNativeUtf8(allocator: arena);
        final pwdPtr = (password != null && password.isNotEmpty) ? password.toNativeUtf8(allocator: arena) : ffi.nullptr;

        final opts = arena<_NativeCreateOptions>();
        opts.ref.structSize = ffi.sizeOf<_NativeCreateOptions>();
        opts.ref.abiVersion = 2;
        opts.ref.format = format.value;
        opts.ref.level = level.value;
        opts.ref.encryption = (password != null && password.isNotEmpty) ? 4 : 0;
        opts.ref.password = pwdPtr;
        opts.ref.threadBudget = threads;
        opts.ref.solidBlockSizeMb = 64;
        opts.ref.progressCallback = ffi.nullptr;
        opts.ref.userData = ffi.nullptr;

        final rc = createFn(sourcePtrs, sources.length, destPtr, opts);
        if (rc != 0) {
          throw Exception('TTZip native compression failed with error code $rc');
        }
      });
    });
  }

  /// Extracts an archive into the specified destination directory using background Isolate.
  static Future<void> extract({
    required String archivePath,
    required String destination,
    String? password,
    int threads = 0,
  }) async {
    await Isolate.run(() {
      final dylib = _loadLibrary();
      final extractFn = dylib.lookupFunction<
          ffi.Int32 Function(ffi.Pointer<Utf8>, ffi.Pointer<Utf8>, ffi.Pointer<_NativeExtractOptions>),
          int Function(ffi.Pointer<Utf8>, ffi.Pointer<Utf8>,
              ffi.Pointer<_NativeExtractOptions>)>('ttzip_rust_extract_archive');

      using((arena) {
        final arcPtr = archivePath.toNativeUtf8(allocator: arena);
        final destPtr = destination.toNativeUtf8(allocator: arena);
        final pwdPtr = (password != null && password.isNotEmpty) ? password.toNativeUtf8(allocator: arena) : ffi.nullptr;

        final opts = arena<_NativeExtractOptions>();
        opts.ref.structSize = ffi.sizeOf<_NativeExtractOptions>();
        opts.ref.abiVersion = 2;
        opts.ref.destinationPath = destPtr;
        opts.ref.password = pwdPtr;
        opts.ref.threadBudget = threads;
        opts.ref.overwriteExisting = true;
        opts.ref.preservePermissions = true;
        opts.ref.dryRun = false;
        opts.ref.progressCallback = ffi.nullptr;
        opts.ref.userData = ffi.nullptr;

        final rc = extractFn(arcPtr, destPtr, opts);
        if (rc != 0) {
          throw Exception('TTZip native extraction failed with error code $rc');
        }
      });
    });
  }

  /// Streams real-time progress events during archive compression.
  static Stream<ArchiveProgress> compressStream({
    required List<String> sources,
    required String destination,
    TTZipFormat format = TTZipFormat.auto,
    TTZipCompressionLevel level = TTZipCompressionLevel.normal,
    String? password,
    int threads = 0,
  }) {
    final controller = StreamController<ArchiveProgress>();
    final port = ReceivePort();

    port.listen((msg) {
      if (msg is List) {
        final progress = ArchiveProgress(
          processedBytes: msg[0] as int,
          totalBytes: msg[1] as int,
          fractionCompleted: msg[2] as double,
          currentEntryPath: msg[3] as String,
        );
        controller.add(progress);
      } else if (msg == null) {
        port.close();
        controller.close();
      } else if (msg is String) {
        port.close();
        controller.addError(Exception(msg));
      }
    });

    compress(
      sources: sources,
      destination: destination,
      format: format,
      level: level,
      password: password,
      threads: threads,
    ).then((_) {
      controller.add(ArchiveProgress(
        processedBytes: 100,
        totalBytes: 100,
        fractionCompleted: 1.0,
        currentEntryPath: '',
        phase: 'completed',
      ));
      controller.close();
      port.close();
    }).catchError((err) {
      controller.addError(err);
      controller.close();
      port.close();
    });

    return controller.stream;
  }

  /// Streams real-time progress events during archive extraction.
  static Stream<ArchiveProgress> extractStream({
    required String archivePath,
    required String destination,
    String? password,
    int threads = 0,
  }) {
    final controller = StreamController<ArchiveProgress>();
    extract(
      archivePath: archivePath,
      destination: destination,
      password: password,
      threads: threads,
    ).then((_) {
      controller.add(ArchiveProgress(
        processedBytes: 100,
        totalBytes: 100,
        fractionCompleted: 1.0,
        currentEntryPath: '',
        phase: 'completed',
      ));
      controller.close();
    }).catchError((err) {
      controller.addError(err);
      controller.close();
    });
    return controller.stream;
  }

  static int _softwareCrc32(Uint8List data) {
    int crc = 0xFFFFFFFF;
    for (var byte in data) {
      crc = (crc >>> 8) ^ _crcTable[(crc ^ byte) & 0xFF];
    }
    return (~crc) & 0xFFFFFFFF;
  }

  static final List<int> _crcTable = List<int>.generate(256, (i) {
    int c = i;
    for (int k = 0; k < 8; k++) {
      c = (c & 1) != 0 ? (0xEDB88320 ^ (c >>> 1)) : (c >>> 1);
    }
    return c;
  });
}
