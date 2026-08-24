// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for .NET 8+.
// Production-grade P/Invoke binding with Spans, SafeHandle, and IAsyncEnumerable.

using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Threading.Channels;
using System.Threading.Tasks;
using Microsoft.Win32.SafeHandles;

namespace TTZip
{
    public enum ArchiveFormat
    {
        Auto = 0,
        Zip = 1,
        SevenZip = 2,
        Tar = 3,
        TarGz = 4,
        TarBz2 = 5,
        TarXz = 6,
        TarZstd = 7,
        Dmg = 8,
        Lzfse = 9,
        Snappy = 10
    }

    public enum CompressionLevel
    {
        Store = 0,
        Fastest = 1,
        Fast = 3,
        Normal = 6,
        Maximum = 9,
        Ultra = 12
    }

    public readonly record struct ArchiveProgress(
        long ProcessedBytes,
        long TotalBytes,
        double FractionCompleted,
        string CurrentEntryPath,
        int CurrentEntryIndex = 0,
        int TotalEntries = 0,
        string Phase = "processing",
        double ThroughputMbs = 0.0
    );

    public readonly record struct EntryMetadata(
        string Path,
        ulong UncompressedSize,
        ulong CompressedSize,
        uint Crc32,
        long MtimeEpochSecs,
        bool IsDirectory,
        bool IsEncrypted
    );

    public sealed class SafeHandleZeroAlloc : SafeHandleZeroOrMinusOneIsInvalid
    {
        public SafeHandleZeroAlloc() : base(true) { }

        public SafeHandleZeroAlloc(IntPtr handle, bool ownsHandle = true) : base(ownsHandle)
        {
            SetHandle(handle);
        }

        protected override bool ReleaseHandle()
        {
            if (handle != IntPtr.Zero)
            {
                Marshal.FreeCoTaskMem(handle);
            }
            return true;
        }
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeCreateOptions
    {
        public uint StructSize;
        public uint AbiVersion;
        public int Format;
        public int Level;
        public int Encryption;
        public IntPtr Password;
        public uint ThreadBudget;
        public uint SolidBlockSizeMb;
        public IntPtr ProgressCallback;
        public IntPtr UserData;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeExtractOptions
    {
        public uint StructSize;
        public uint AbiVersion;
        public IntPtr DestinationPath;
        public IntPtr Password;
        public uint ThreadBudget;
        [MarshalAs(UnmanagedType.I1)]
        public bool OverwriteExisting;
        [MarshalAs(UnmanagedType.I1)]
        public bool PreservePermissions;
        [MarshalAs(UnmanagedType.I1)]
        public bool DryRun;
        public IntPtr ProgressCallback;
        public IntPtr UserData;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeEntryMetadata
    {
        public uint StructSize;
        public uint AbiVersion;
        public IntPtr Path;
        public ulong UncompressedSize;
        public ulong CompressedSize;
        public uint Crc32;
        public long MtimeEpochSecs;
        public uint Mode;
        [MarshalAs(UnmanagedType.I1)]
        public bool IsDirectory;
        [MarshalAs(UnmanagedType.I1)]
        public bool IsEncrypted;
        public ushort CompressionMethod;
        public IntPtr DetectedEncoding;
    }

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    internal delegate bool NativeProgressCallback(ulong processedBytes, ulong totalBytes, IntPtr currentEntry, IntPtr userData);

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    internal delegate bool NativeInspectCallback(IntPtr metaPtr, IntPtr userData);

    public static class TTZipEngine
    {
        private const string LibName = "ttzip_glue";
        public const uint AbiVersion2 = 2;

        [DllImport(LibName, EntryPoint = "ttzip_rust_version", CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr ttzip_rust_version();

        [DllImport(LibName, EntryPoint = "ttzip_rust_is_hardware_accelerated", CallingConvention = CallingConvention.Cdecl)]
        [return: MarshalAs(UnmanagedType.I1)]
        private static extern bool ttzip_rust_is_hardware_accelerated();

        [DllImport(LibName, EntryPoint = "ttzip_rust_crc32", CallingConvention = CallingConvention.Cdecl)]
        private static extern uint ttzip_rust_crc32(uint crc, IntPtr data, nuint len);

        [DllImport(LibName, EntryPoint = "ttzip_rust_crc64", CallingConvention = CallingConvention.Cdecl)]
        private static extern ulong ttzip_rust_crc64(ulong seed, IntPtr data, nuint len);

        [DllImport(LibName, EntryPoint = "ttzip_rust_create_archive", CallingConvention = CallingConvention.Cdecl)]
        private static extern int ttzip_rust_create_archive(IntPtr[] sourcePaths, nuint sourceCount, IntPtr destinationPath, ref NativeCreateOptions options);

        [DllImport(LibName, EntryPoint = "ttzip_rust_extract_archive", CallingConvention = CallingConvention.Cdecl)]
        private static extern int ttzip_rust_extract_archive(IntPtr archivePath, IntPtr destinationPath, ref NativeExtractOptions options);

        [DllImport(LibName, EntryPoint = "ttzip_rust_inspect_archive", CallingConvention = CallingConvention.Cdecl)]
        private static extern int ttzip_rust_inspect_archive(IntPtr archivePath, IntPtr password, [MarshalAs(UnmanagedType.I1)] bool detectEncoding, NativeInspectCallback callback, IntPtr userData);

        public static string Version
        {
            get
            {
                try
                {
                    IntPtr ptr = ttzip_rust_version();
                    return ptr != IntPtr.Zero ? Marshal.PtrToStringUTF8(ptr) ?? "1.0.0" : "1.0.0";
                }
                catch
                {
                    return "1.0.0";
                }
            }
        }

        public static bool IsHardwareAccelerated
        {
            get
            {
                try
                {
                    return ttzip_rust_is_hardware_accelerated();
                }
                catch
                {
                    return false;
                }
            }
        }

        /// <summary>
        /// Computes SIMD-accelerated CRC-32 (>40 GB/s on Apple Silicon / AVX-512) over a ReadOnlySpan.
        /// </summary>
        public static unsafe uint ComputeCrc32(ReadOnlySpan<byte> data, uint seed = 0)
        {
            if (data.IsEmpty) return 0;
            fixed (byte* p = data)
            {
                return ttzip_rust_crc32(seed, (IntPtr)p, (nuint)data.Length);
            }
        }

        /// <summary>
        /// Computes SIMD-accelerated CRC-64 over a ReadOnlySpan.
        /// </summary>
        public static unsafe ulong ComputeCrc64(ReadOnlySpan<byte> data, ulong seed = 0)
        {
            if (data.IsEmpty) return 0;
            fixed (byte* p = data)
            {
                return ttzip_rust_crc64(seed, (IntPtr)p, (nuint)data.Length);
            }
        }

        /// <summary>
        /// Synchronously creates an archive from source files/directories.
        /// </summary>
        public static void CreateArchive(
            string[] sourcePaths,
            string destinationPath,
            ArchiveFormat format = ArchiveFormat.Zip,
            CompressionLevel level = CompressionLevel.Normal,
            string? password = null,
            int threads = 0
        )
        {
            if (sourcePaths == null || sourcePaths.Length == 0)
                throw new ArgumentException("Sources cannot be empty", nameof(sourcePaths));

            IntPtr[] nativeSources = new IntPtr[sourcePaths.Length];
            IntPtr destPtr = IntPtr.Zero;
            IntPtr pwdPtr = IntPtr.Zero;

            try
            {
                for (int i = 0; i < sourcePaths.Length; i++)
                {
                    nativeSources[i] = Marshal.StringToCoTaskMemUTF8(sourcePaths[i]);
                }
                destPtr = Marshal.StringToCoTaskMemUTF8(destinationPath);
                if (!string.IsNullOrEmpty(password))
                {
                    pwdPtr = Marshal.StringToCoTaskMemUTF8(password);
                }

                NativeCreateOptions opts = new NativeCreateOptions
                {
                    StructSize = (uint)Marshal.SizeOf<NativeCreateOptions>(),
                    AbiVersion = AbiVersion2,
                    Format = (int)format,
                    Level = (int)level,
                    Encryption = pwdPtr != IntPtr.Zero ? 4 : 0,
                    Password = pwdPtr,
                    ThreadBudget = (uint)Math.Max(0, threads),
                    SolidBlockSizeMb = 64,
                    ProgressCallback = IntPtr.Zero,
                    UserData = IntPtr.Zero
                };

                int rc = ttzip_rust_create_archive(nativeSources, (nuint)sourcePaths.Length, destPtr, ref opts);
                if (rc != 0)
                {
                    throw new InvalidOperationException($"TTZip native archive creation failed with status code {rc}");
                }
            }
            finally
            {
                foreach (var ptr in nativeSources)
                {
                    if (ptr != IntPtr.Zero) Marshal.FreeCoTaskMem(ptr);
                }
                if (destPtr != IntPtr.Zero) Marshal.FreeCoTaskMem(destPtr);
                if (pwdPtr != IntPtr.Zero) Marshal.FreeCoTaskMem(pwdPtr);
            }
        }

        /// <summary>
        /// Asynchronously creates an archive yielding real-time IAsyncEnumerable progress events.
        /// </summary>
        public static async IAsyncEnumerable<ArchiveProgress> CreateArchiveAsync(
            string[] sourcePaths,
            string destinationPath,
            ArchiveFormat format = ArchiveFormat.Zip,
            CompressionLevel level = CompressionLevel.Normal,
            string? password = null,
            int threads = 0,
            [EnumeratorCancellation] CancellationToken cancellationToken = default
        )
        {
            var channel = Channel.CreateUnbounded<ArchiveProgress>(new UnboundedChannelOptions { SingleWriter = true });

            _ = Task.Run(() =>
            {
                IntPtr[] nativeSources = new IntPtr[sourcePaths.Length];
                IntPtr destPtr = IntPtr.Zero;
                IntPtr pwdPtr = IntPtr.Zero;

                try
                {
                    for (int i = 0; i < sourcePaths.Length; i++)
                    {
                        nativeSources[i] = Marshal.StringToCoTaskMemUTF8(sourcePaths[i]);
                    }
                    destPtr = Marshal.StringToCoTaskMemUTF8(destinationPath);
                    if (!string.IsNullOrEmpty(password))
                    {
                        pwdPtr = Marshal.StringToCoTaskMemUTF8(password);
                    }

                    NativeProgressCallback cb = (processed, total, currentEntry, userData) =>
                    {
                        string entry = currentEntry != IntPtr.Zero ? Marshal.PtrToStringUTF8(currentEntry) ?? "" : "";
                        double frac = total > 0 ? (double)processed / total : 0.0;
                        channel.Writer.TryWrite(new ArchiveProgress((long)processed, (long)total, frac, entry));
                        return !cancellationToken.IsCancellationRequested;
                    };

                    NativeCreateOptions opts = new NativeCreateOptions
                    {
                        StructSize = (uint)Marshal.SizeOf<NativeCreateOptions>(),
                        AbiVersion = AbiVersion2,
                        Format = (int)format,
                        Level = (int)level,
                        Encryption = pwdPtr != IntPtr.Zero ? 4 : 0,
                        Password = pwdPtr,
                        ThreadBudget = (uint)Math.Max(0, threads),
                        SolidBlockSizeMb = 64,
                        ProgressCallback = Marshal.GetFunctionPointerForDelegate(cb),
                        UserData = IntPtr.Zero
                    };

                    int rc = ttzip_rust_create_archive(nativeSources, (nuint)sourcePaths.Length, destPtr, ref opts);
                    GC.KeepAlive(cb);

                    if (rc != 0)
                    {
                        channel.Writer.Complete(new InvalidOperationException($"Archive creation failed: {rc}"));
                    }
                    else
                    {
                        channel.Writer.Complete();
                    }
                }
                catch (Exception ex)
                {
                    channel.Writer.Complete(ex);
                }
                finally
                {
                    foreach (var ptr in nativeSources)
                    {
                        if (ptr != IntPtr.Zero) Marshal.FreeCoTaskMem(ptr);
                    }
                    if (destPtr != IntPtr.Zero) Marshal.FreeCoTaskMem(destPtr);
                    if (pwdPtr != IntPtr.Zero) Marshal.FreeCoTaskMem(pwdPtr);
                }
            }, cancellationToken);

            while (await channel.Reader.WaitToReadAsync(cancellationToken).ConfigureAwait(false))
            {
                while (channel.Reader.TryRead(out var progress))
                {
                    yield return progress;
                }
            }
        }

        /// <summary>
        /// Synchronously extracts an archive to destination directory.
        /// </summary>
        public static void ExtractArchive(
            string archivePath,
            string destinationPath,
            string? password = null,
            int threads = 0
        )
        {
            IntPtr arcPtr = IntPtr.Zero;
            IntPtr destPtr = IntPtr.Zero;
            IntPtr pwdPtr = IntPtr.Zero;

            try
            {
                arcPtr = Marshal.StringToCoTaskMemUTF8(archivePath);
                destPtr = Marshal.StringToCoTaskMemUTF8(destinationPath);
                if (!string.IsNullOrEmpty(password))
                {
                    pwdPtr = Marshal.StringToCoTaskMemUTF8(password);
                }

                NativeExtractOptions opts = new NativeExtractOptions
                {
                    StructSize = (uint)Marshal.SizeOf<NativeExtractOptions>(),
                    AbiVersion = AbiVersion2,
                    DestinationPath = destPtr,
                    Password = pwdPtr,
                    ThreadBudget = (uint)Math.Max(0, threads),
                    OverwriteExisting = true,
                    PreservePermissions = true,
                    DryRun = false,
                    ProgressCallback = IntPtr.Zero,
                    UserData = IntPtr.Zero
                };

                int rc = ttzip_rust_extract_archive(arcPtr, destPtr, ref opts);
                if (rc != 0)
                {
                    throw new InvalidOperationException($"TTZip native archive extraction failed with status code {rc}");
                }
            }
            finally
            {
                if (arcPtr != IntPtr.Zero) Marshal.FreeCoTaskMem(arcPtr);
                if (destPtr != IntPtr.Zero) Marshal.FreeCoTaskMem(destPtr);
                if (pwdPtr != IntPtr.Zero) Marshal.FreeCoTaskMem(pwdPtr);
            }
        }

        /// <summary>
        /// Asynchronously extracts an archive yielding real-time IAsyncEnumerable progress events.
        /// </summary>
        public static async IAsyncEnumerable<ArchiveProgress> ExtractArchiveAsync(
            string archivePath,
            string destinationPath,
            string? password = null,
            int threads = 0,
            [EnumeratorCancellation] CancellationToken cancellationToken = default
        )
        {
            var channel = Channel.CreateUnbounded<ArchiveProgress>(new UnboundedChannelOptions { SingleWriter = true });

            _ = Task.Run(() =>
            {
                IntPtr arcPtr = IntPtr.Zero;
                IntPtr destPtr = IntPtr.Zero;
                IntPtr pwdPtr = IntPtr.Zero;

                try
                {
                    arcPtr = Marshal.StringToCoTaskMemUTF8(archivePath);
                    destPtr = Marshal.StringToCoTaskMemUTF8(destinationPath);
                    if (!string.IsNullOrEmpty(password))
                    {
                        pwdPtr = Marshal.StringToCoTaskMemUTF8(password);
                    }

                    NativeProgressCallback cb = (processed, total, currentEntry, userData) =>
                    {
                        string entry = currentEntry != IntPtr.Zero ? Marshal.PtrToStringUTF8(currentEntry) ?? "" : "";
                        double frac = total > 0 ? (double)processed / total : 0.0;
                        channel.Writer.TryWrite(new ArchiveProgress((long)processed, (long)total, frac, entry));
                        return !cancellationToken.IsCancellationRequested;
                    };

                    NativeExtractOptions opts = new NativeExtractOptions
                    {
                        StructSize = (uint)Marshal.SizeOf<NativeExtractOptions>(),
                        AbiVersion = AbiVersion2,
                        DestinationPath = destPtr,
                        Password = pwdPtr,
                        ThreadBudget = (uint)Math.Max(0, threads),
                        OverwriteExisting = true,
                        PreservePermissions = true,
                        DryRun = false,
                        ProgressCallback = Marshal.GetFunctionPointerForDelegate(cb),
                        UserData = IntPtr.Zero
                    };

                    int rc = ttzip_rust_extract_archive(arcPtr, destPtr, ref opts);
                    GC.KeepAlive(cb);

                    if (rc != 0)
                    {
                        channel.Writer.Complete(new InvalidOperationException($"Archive extraction failed: {rc}"));
                    }
                    else
                    {
                        channel.Writer.Complete();
                    }
                }
                catch (Exception ex)
                {
                    channel.Writer.Complete(ex);
                }
                finally
                {
                    if (arcPtr != IntPtr.Zero) Marshal.FreeCoTaskMem(arcPtr);
                    if (destPtr != IntPtr.Zero) Marshal.FreeCoTaskMem(destPtr);
                    if (pwdPtr != IntPtr.Zero) Marshal.FreeCoTaskMem(pwdPtr);
                }
            }, cancellationToken);

            while (await channel.Reader.WaitToReadAsync(cancellationToken).ConfigureAwait(false))
            {
                while (channel.Reader.TryRead(out var progress))
                {
                    yield return progress;
                }
            }
        }

        /// <summary>
        /// Inspects entry metadata in an archive without disk extraction.
        /// </summary>
        public static List<EntryMetadata> InspectArchive(string archivePath, string? password = null)
        {
            IntPtr arcPtr = IntPtr.Zero;
            IntPtr pwdPtr = IntPtr.Zero;
            List<EntryMetadata> list = new List<EntryMetadata>();

            try
            {
                arcPtr = Marshal.StringToCoTaskMemUTF8(archivePath);
                if (!string.IsNullOrEmpty(password))
                {
                    pwdPtr = Marshal.StringToCoTaskMemUTF8(password);
                }

                NativeInspectCallback cb = (metaPtr, userData) =>
                {
                    if (metaPtr == IntPtr.Zero) return false;
                    NativeEntryMetadata meta = Marshal.PtrToStructure<NativeEntryMetadata>(metaPtr);
                    string path = meta.Path != IntPtr.Zero ? Marshal.PtrToStringUTF8(meta.Path) ?? "" : "";
                    list.Add(new EntryMetadata(
                        path,
                        meta.UncompressedSize,
                        meta.CompressedSize,
                        meta.Crc32,
                        meta.MtimeEpochSecs,
                        meta.IsDirectory,
                        meta.IsEncrypted
                    ));
                    return true;
                };

                int rc = ttzip_rust_inspect_archive(arcPtr, pwdPtr, true, cb, IntPtr.Zero);
                GC.KeepAlive(cb);

                if (rc != 0)
                {
                    throw new InvalidOperationException($"Archive inspection failed with status code {rc}");
                }
            }
            finally
            {
                if (arcPtr != IntPtr.Zero) Marshal.FreeCoTaskMem(arcPtr);
                if (pwdPtr != IntPtr.Zero) Marshal.FreeCoTaskMem(pwdPtr);
            }

            return list;
        }
    }
}
