// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for .NET 8+.
// Multi-RID dynamic library resolver and DllImport hook.

using System;
using System.IO;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace TTZip.Native
{
    /// <summary>
    /// Custom NativeLibrary resolver for TTZip supporting multi-RID asset packaging and bundle lookups.
    /// </summary>
    public static class NativeResolver
    {
        private static bool _isInitialized;
        private static readonly object _initLock = new();

        /// <summary>
        /// Automatically registers the DLL import resolver when the assembly is loaded in .NET 8+.
        /// </summary>
        [ModuleInitializer]
        public static void Initialize()
        {
            EnsureInitialized();
        }

        /// <summary>
        /// Ensures NativeLibrary.SetDllImportResolver is registered for the TTZip assembly.
        /// </summary>
        public static void EnsureInitialized()
        {
            if (_isInitialized) return;

            lock (_initLock)
            {
                if (_isInitialized) return;

                NativeLibrary.SetDllImportResolver(typeof(NativeResolver).Assembly, ResolveDllImport);
                _isInitialized = true;
            }
        }

        private static IntPtr ResolveDllImport(string libraryName, Assembly assembly, DllImportSearchPath? searchPath)
        {
            if (libraryName != "ttzip_engine" && libraryName != "ttzip_glue" && libraryName != "ttzip"
                && libraryName != "libttzip_engine" && libraryName != "libttzip_glue")
            {
                return IntPtr.Zero;
            }

            // 1. Check environment variable overrides
            string? envPath = Environment.GetEnvironmentVariable("TTZIP_DYLIB_PATH")
                           ?? Environment.GetEnvironmentVariable("TTZIP_LIB_PATH")
                           ?? Environment.GetEnvironmentVariable("LIBTTZIP_PATH");

            if (!string.IsNullOrEmpty(envPath) && File.Exists(envPath))
            {
                if (NativeLibrary.TryLoad(envPath, out IntPtr envHandle))
                {
                    return envHandle;
                }
            }

            // 2. Multi-RID platform detection
            string osPrefix = RuntimeInformation.IsOSPlatform(OSPlatform.OSX) ? "osx"
                            : RuntimeInformation.IsOSPlatform(OSPlatform.Linux) ? "linux"
                            : RuntimeInformation.IsOSPlatform(OSPlatform.Windows) ? "win"
                            : "unknown";

            string arch = RuntimeInformation.ProcessArchitecture switch
            {
                Architecture.Arm64 => "arm64",
                Architecture.X64 => "x64",
                Architecture.X86 => "x86",
                Architecture.Arm => "arm",
                _ => "x64"
            };

            string rid = $"{osPrefix}-{arch}";
            string ext = osPrefix switch
            {
                "osx" => ".dylib",
                "linux" => ".so",
                "win" => ".dll",
                _ => ".dylib"
            };

            string[] fileNames = osPrefix switch
            {
                "win" => new[] { "ttzip_engine.dll", "ttzip_glue.dll", "libttzip_engine.dll" },
                _ => new[] { $"libttzip_engine{ext}", $"libttzip_glue{ext}", $"ttzip_engine{ext}" }
            };

            string baseDir = AppContext.BaseDirectory;
            string? assemblyDir = Path.GetDirectoryName(assembly.Location);

            // Candidate directories
            string[] searchDirs = new[]
            {
                Path.Combine(baseDir, "runtimes", rid, "native"),
                baseDir,
                !string.IsNullOrEmpty(assemblyDir) ? Path.Combine(assemblyDir, "runtimes", rid, "native") : "",
                !string.IsNullOrEmpty(assemblyDir) ? assemblyDir : "",
                // Relative paths to repository workspace build target
                Path.Combine(baseDir, "..", "..", "..", "..", "rust", "target", "release"),
                Path.Combine(baseDir, "..", "..", "..", "rust", "target", "release"),
                Path.Combine(baseDir, "..", "..", "rust", "target", "release"),
                Path.Combine(baseDir, "..", "rust", "target", "release"),
                Path.Combine(baseDir, "rust", "target", "release"),
                Path.Combine(Directory.GetCurrentDirectory(), "rust", "target", "release"),
                Path.Combine(Directory.GetCurrentDirectory(), "..", "rust", "target", "release"),
                // System standard paths
                "/opt/homebrew/lib",
                "/usr/local/lib",
                "/usr/lib"
            };

            foreach (string dir in searchDirs)
            {
                if (string.IsNullOrEmpty(dir) || !Directory.Exists(dir)) continue;

                foreach (string fileName in fileNames)
                {
                    string candidate = Path.Combine(dir, fileName);
                    if (File.Exists(candidate))
                    {
                        if (NativeLibrary.TryLoad(candidate, out IntPtr handle))
                        {
                            return handle;
                        }
                    }
                }
            }

            // 3. Fallback default system load
            foreach (string fileName in fileNames)
            {
                if (NativeLibrary.TryLoad(fileName, assembly, searchPath, out IntPtr handle))
                {
                    return handle;
                }
            }

            return IntPtr.Zero;
        }
    }
}
