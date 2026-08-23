// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.

using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

namespace TTZip
{
    public enum ArchiveFormat
    {
        Auto = 0,
        Zip = 1,
        SevenZip = 2,
        Tar = 3
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

    public struct EntryMetadata
    {
        public string Path;
        public ulong UncompressedSize;
        public ulong CompressedSize;
        public uint Crc32;
        public long MtimeEpochSecs;
        public bool IsDirectory;
        public bool IsEncrypted;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeCreateOptions
    {
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
    }

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    internal delegate bool NativeInspectCallback(IntPtr metaPtr, IntPtr userData);

    public static class TTZipEngine
    {
        private const string LibName = "ttzip_glue";

        [DllImport(LibName, EntryPoint = "ttzip_rust_version", CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr ttzip_rust_version();

        [DllImport(LibName, EntryPoint = "ttzip_rust_crc32", CallingConvention = CallingConvention.Cdecl)]
        private static extern uint ttzip_rust_crc32(uint crc, byte[] data, nuint len);

        [DllImport(LibName, EntryPoint = "ttzip_rust_create_archive", CallingConvention = CallingConvention.Cdecl)]
        private static extern int ttzip_rust_create_archive(IntPtr[] sourcePaths, nuint sourceCount, string destinationPath, ref NativeCreateOptions options);

        [DllImport(LibName, EntryPoint = "ttzip_rust_extract_archive", CallingConvention = CallingConvention.Cdecl)]
        private static extern int ttzip_rust_extract_archive(string archivePath, string destinationPath, ref NativeExtractOptions options);

        public static string GetVersion()
        {
            IntPtr ptr = ttzip_rust_version();
            return ptr != IntPtr.Zero ? Marshal.PtrToStringUTF8(ptr) : "1.0.0";
        }

        public static uint ComputeCrc32(byte[] data)
        {
            if (data == null) throw new ArgumentNullException(nameof(data));
            return ttzip_rust_crc32(0, data, (nuint)data.Length);
        }

        public static void CreateArchive(string[] sourcePaths, string destinationPath, CompressionLevel level = CompressionLevel.Normal)
        {
            if (sourcePaths == null || sourcePaths.Length == 0) throw new ArgumentException("Sources cannot be empty");
            
            IntPtr[] nativePaths = new IntPtr[sourcePaths.Length];
            try
            {
                for (int i = 0; i < sourcePaths.Length; i++)
                {
                    nativePaths[i] = Marshal.StringToHGlobalAnsi(sourcePaths[i]);
                }

                NativeCreateOptions opts = new NativeCreateOptions
                {
                    Format = (int)ArchiveFormat.Zip,
                    Level = (int)level
                };

                int rc = ttzip_rust_create_archive(nativePaths, (nuint)sourcePaths.Length, destinationPath, ref opts);
                if (rc != 0) throw new InvalidOperationException($"Failed to create archive with error code {rc}");
            }
            finally
            {
                foreach (var ptr in nativePaths)
                {
                    if (ptr != IntPtr.Zero) Marshal.FreeHGlobal(ptr);
                }
            }
        }

        public static void ExtractArchive(string archivePath, string destinationPath)
        {
            NativeExtractOptions opts = new NativeExtractOptions
            {
                DestinationPath = Marshal.StringToHGlobalAnsi(destinationPath),
                OverwriteExisting = true
            };

            try
            {
                int rc = ttzip_rust_extract_archive(archivePath, destinationPath, ref opts);
                if (rc != 0) throw new InvalidOperationException($"Failed to extract archive with error code {rc}");
            }
            finally
            {
                if (opts.DestinationPath != IntPtr.Zero) Marshal.FreeHGlobal(opts.DestinationPath);
            }
        }
    }
}
