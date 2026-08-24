// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for Dart & Flutter.
// Multi-platform dynamic library resolver with environment and bundle fallbacks.

import 'dart:ffi' as ffi;
import 'dart:io' show Directory, File, Platform;

/// Resolves and loads the TTZip native dynamic library across platforms.
ffi.DynamicLibrary loadTTZipLibrary() {
  // 1. Check explicit environment variable overrides
  final envKeys = ['TTZIP_DYLIB_PATH', 'TTZIP_LIB_PATH', 'LIBTTZIP_PATH'];
  for (final key in envKeys) {
    final envPath = Platform.environment[key];
    if (envPath != null && envPath.isNotEmpty) {
      if (File(envPath).existsSync()) {
        try {
          return ffi.DynamicLibrary.open(envPath);
        } catch (_) {}
      }
    }
  }

  // 2. Platform-specific dynamic library candidate paths
  if (Platform.isMacOS) {
    const candidates = [
      // App bundle & Flutter framework bundle paths
      'Frameworks/TTZipVendor.framework/TTZipVendor',
      '../Frameworks/TTZipVendor.framework/TTZipVendor',
      'TTZipVendor.framework/TTZipVendor',
      'TTZipCore.framework/TTZipCore',
      // Local workspace / build tree relative paths
      'rust/target/release/libttzip_engine.dylib',
      'rust/target/release/libttzip_glue.dylib',
      '../rust/target/release/libttzip_engine.dylib',
      '../rust/target/release/libttzip_glue.dylib',
      '../../rust/target/release/libttzip_engine.dylib',
      '../../rust/target/release/libttzip_glue.dylib',
      '../../../rust/target/release/libttzip_engine.dylib',
      '../../../rust/target/release/libttzip_glue.dylib',
      '../../../../rust/target/release/libttzip_engine.dylib',
      // Working directory / system paths
      'libttzip_engine.dylib',
      'libttzip_glue.dylib',
      '/opt/homebrew/lib/libttzip_engine.dylib',
      '/usr/local/lib/libttzip_engine.dylib',
    ];

    for (final path in candidates) {
      if (File(path).existsSync()) {
        try {
          return ffi.DynamicLibrary.open(path);
        } catch (_) {}
      }
    }

    try {
      return ffi.DynamicLibrary.open('libttzip_engine.dylib');
    } catch (_) {}

    try {
      return ffi.DynamicLibrary.process();
    } catch (_) {
      return ffi.DynamicLibrary.executable();
    }
  } else if (Platform.isIOS) {
    const candidates = [
      'TTZipVendor.framework/TTZipVendor',
      'Frameworks/TTZipVendor.framework/TTZipVendor',
      'TTZipCore.framework/TTZipCore',
    ];

    for (final path in candidates) {
      try {
        return ffi.DynamicLibrary.open(path);
      } catch (_) {}
    }

    try {
      return ffi.DynamicLibrary.process();
    } catch (_) {
      return ffi.DynamicLibrary.executable();
    }
  } else if (Platform.isLinux || Platform.isAndroid) {
    const candidates = [
      'rust/target/release/libttzip_engine.so',
      'rust/target/release/libttzip_glue.so',
      '../rust/target/release/libttzip_engine.so',
      '../rust/target/release/libttzip_glue.so',
      '../../rust/target/release/libttzip_engine.so',
      '../../rust/target/release/libttzip_glue.so',
      '../../../rust/target/release/libttzip_engine.so',
      '../../../rust/target/release/libttzip_glue.so',
      '../../../../rust/target/release/libttzip_engine.so',
      'libttzip_engine.so',
      'libttzip_glue.so',
      '/usr/local/lib/libttzip_engine.so',
      '/usr/lib/libttzip_engine.so',
    ];

    for (final path in candidates) {
      if (File(path).existsSync()) {
        try {
          return ffi.DynamicLibrary.open(path);
        } catch (_) {}
      }
    }

    try {
      return ffi.DynamicLibrary.open('libttzip_engine.so');
    } catch (_) {}

    try {
      return ffi.DynamicLibrary.open('libttzip_glue.so');
    } catch (_) {}

    try {
      return ffi.DynamicLibrary.process();
    } catch (_) {
      return ffi.DynamicLibrary.executable();
    }
  } else if (Platform.isWindows) {
    const candidates = [
      'rust/target/release/ttzip_engine.dll',
      'rust/target/release/ttzip_glue.dll',
      '../rust/target/release/ttzip_engine.dll',
      '../rust/target/release/ttzip_glue.dll',
      '../../rust/target/release/ttzip_engine.dll',
      '../../rust/target/release/ttzip_glue.dll',
      '../../../rust/target/release/ttzip_engine.dll',
      '../../../rust/target/release/ttzip_glue.dll',
      '../../../../rust/target/release/ttzip_engine.dll',
      'ttzip_engine.dll',
      'ttzip_glue.dll',
    ];

    for (final path in candidates) {
      if (File(path).existsSync()) {
        try {
          return ffi.DynamicLibrary.open(path);
        } catch (_) {}
      }
    }

    try {
      return ffi.DynamicLibrary.open('ttzip_engine.dll');
    } catch (_) {}

    try {
      return ffi.DynamicLibrary.open('ttzip_glue.dll');
    } catch (_) {}

    return ffi.DynamicLibrary.process();
  }

  return ffi.DynamicLibrary.process();
}

/// Safely attempts to load the TTZip dynamic library, returning null if unresolved.
ffi.DynamicLibrary? tryLoadTTZipLibrary() {
  try {
    return loadTTZipLibrary();
  } catch (_) {
    return null;
  }
}
