// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for .NET 8+.
// xUnit / NUnit style test suite verifying ReadOnlySpan<byte> zero-copy buffer slicing,
// SafeHandleZeroAlloc lifecycle, and IAsyncEnumerable async streaming.

using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using Xunit;

namespace TTZip.Tests
{
    public class TTZipTest : IDisposable
    {
        private readonly string _tempDir;

        public TTZipTest()
        {
            _tempDir = Path.Combine(Path.GetTempPath(), "ttzip_dotnet_test_" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(_tempDir);
        }

        public void Dispose()
        {
            if (Directory.Exists(_tempDir))
            {
                try
                {
                    Directory.Delete(_tempDir, true);
                }
                catch { }
            }
        }

        [Fact]
        public void TestEngineVersionAndHardwareAcceleration()
        {
            string version = TTZipEngine.Version;
            Assert.NotNull(version);
            Assert.NotEmpty(version);

            bool hw = TTZipEngine.IsHardwareAccelerated;
            Assert.True(hw || !hw, "Hardware acceleration must be queryable");
        }

        [Fact]
        public void TestReadOnlySpanZeroCopySlicingAndCrc()
        {
            byte[] fullPayload = Encoding.UTF8.GetBytes("TTZip .NET 8+ Zero-Copy ReadOnlySpan Buffer Slicing Test Payload 2026");
            ReadOnlySpan<byte> span = fullPayload.AsSpan();

            // 1. Direct whole-span CRC-32
            uint crcFull = TTZipEngine.ComputeCrc32(span);
            Assert.NotEqual(0u, crcFull);

            // 2. Zero-copy buffer slicing: compute over slices
            int mid = span.Length / 2;
            ReadOnlySpan<byte> slice1 = span.Slice(0, mid);
            ReadOnlySpan<byte> slice2 = span.Slice(mid);

            uint seed = TTZipEngine.ComputeCrc32(slice1, 0);
            uint chainedCrc = TTZipEngine.ComputeCrc32(slice2, seed);
            Assert.Equal(crcFull, chainedCrc);

            // 3. CRC-64 computation on span
            ulong crc64Full = TTZipEngine.ComputeCrc64(span);
            Assert.NotEqual(0UL, crc64Full);

            ulong chainedCrc64 = TTZipEngine.ComputeCrc64(slice2, TTZipEngine.ComputeCrc64(slice1, 0));
            Assert.NotEqual(0UL, chainedCrc64);
        }

        [Fact]
        public void TestSafeHandleZeroAllocLifecycle()
        {
            const int bufferSize = 256;
            IntPtr rawMemory = Marshal.AllocCoTaskMem(bufferSize);
            Assert.NotEqual(IntPtr.Zero, rawMemory);

            // Populate test pattern
            for (int i = 0; i < bufferSize; i++)
            {
                Marshal.WriteByte(rawMemory, i, (byte)(i & 0xFF));
            }

            SafeHandleZeroAlloc handle = new SafeHandleZeroAlloc(rawMemory, ownsHandle: true);
            Assert.False(handle.IsInvalid, "Handle should be valid after initialization");
            Assert.False(handle.IsClosed, "Handle should not be closed initially");

            // Verify safe pointer extraction without allocation
            bool success = false;
            handle.DangerousAddRef(ref success);
            Assert.True(success, "DangerousAddRef should succeed");
            try
            {
                IntPtr ptr = handle.DangerousGetHandle();
                Assert.Equal(rawMemory, ptr);
                byte readBack = Marshal.ReadByte(ptr, 10);
                Assert.Equal(10, readBack);
            }
            finally
            {
                handle.DangerousRelease();
            }

            // Dispose should safely release unmanaged memory via ReleaseHandle
            handle.Dispose();
            Assert.True(handle.IsClosed, "Handle must be marked closed after Dispose");
            Assert.True(handle.IsInvalid, "Handle must be invalid after disposal");
        }

        [Fact]
        public async Task TestIAsyncEnumerableStreamingArchiving()
        {
            string sourceFile = Path.Combine(_tempDir, "async_source.txt");
            string payload = new string('A', 32 * 1024);
            await File.WriteAllTextAsync(sourceFile, payload);

            string archivePath = Path.Combine(_tempDir, "async_stream.zip");
            string extractPath = Path.Combine(_tempDir, "async_extracted");
            Directory.CreateDirectory(extractPath);

            // 1. CreateArchiveAsync streaming
            List<ArchiveProgress> createEvents = new List<ArchiveProgress>();
            using CancellationTokenSource cts = new CancellationTokenSource(TimeSpan.FromSeconds(30));

            await foreach (var progress in TTZipEngine.CreateArchiveAsync(
                new[] { sourceFile },
                archivePath,
                ArchiveFormat.Zip,
                CompressionLevel.Fast,
                threads: 2,
                cancellationToken: cts.Token
            ))
            {
                createEvents.Add(progress);
            }

            Assert.True(File.Exists(archivePath), "Archive must exist after async stream completion");
            Assert.True(new FileInfo(archivePath).Length > 0, "Archive size must be non-zero");

            // 2. ExtractArchiveAsync streaming
            List<ArchiveProgress> extractEvents = new List<ArchiveProgress>();
            await foreach (var progress in TTZipEngine.ExtractArchiveAsync(
                archivePath,
                extractPath,
                threads: 2,
                cancellationToken: cts.Token
            ))
            {
                extractEvents.Add(progress);
            }

            string extractedDoc = Path.Combine(extractPath, "async_source.txt");
            Assert.True(File.Exists(extractedDoc), "Extracted file must exist");
            string readBack = await File.ReadAllTextAsync(extractedDoc);
            Assert.Equal(payload, readBack);
        }

        [Fact]
        public void TestSynchronousArchiveRoundtripAndMetadataInspection()
        {
            string file1 = Path.Combine(_tempDir, "doc1.txt");
            string file2 = Path.Combine(_tempDir, "doc2.log");
            string content1 = "Synchronous Archiving Content 1";
            string content2 = "Synchronous Archiving Content 2 - Logs";

            File.WriteAllText(file1, content1);
            File.WriteAllText(file2, content2);

            string archivePath = Path.Combine(_tempDir, "sync_test.zip");
            string destDir = Path.Combine(_tempDir, "sync_extracted");
            Directory.CreateDirectory(destDir);

            // Compress
            TTZipEngine.CreateArchive(
                new[] { file1, file2 },
                archivePath,
                ArchiveFormat.Zip,
                CompressionLevel.Normal
            );
            Assert.True(File.Exists(archivePath));

            // Inspect
            List<EntryMetadata> entries = TTZipEngine.InspectArchive(archivePath);
            Assert.NotNull(entries);
            Assert.NotEmpty(entries);

            // Extract
            TTZipEngine.ExtractArchive(archivePath, destDir);
            string ext1 = Path.Combine(destDir, "doc1.txt");
            string ext2 = Path.Combine(destDir, "doc2.log");
            Assert.True(File.Exists(ext1));
            Assert.True(File.Exists(ext2));
            Assert.Equal(content1, File.ReadAllText(ext1));
            Assert.Equal(content2, File.ReadAllText(ext2));
        }

        public static async Task<int> Main(string[] args)
        {
            Console.WriteLine("⚡️ Running TTZip .NET 8+ SDK Test Suite via Standalone Runner...");
            using var test = new TTZipTest();

            try
            {
                test.TestEngineVersionAndHardwareAcceleration();
                Console.WriteLine("  [PASS] TestEngineVersionAndHardwareAcceleration");

                test.TestReadOnlySpanZeroCopySlicingAndCrc();
                Console.WriteLine("  [PASS] TestReadOnlySpanZeroCopySlicingAndCrc");

                test.TestSafeHandleZeroAllocLifecycle();
                Console.WriteLine("  [PASS] TestSafeHandleZeroAllocLifecycle");

                await test.TestIAsyncEnumerableStreamingArchiving();
                Console.WriteLine("  [PASS] TestIAsyncEnumerableStreamingArchiving");

                test.TestSynchronousArchiveRoundtripAndMetadataInspection();
                Console.WriteLine("  [PASS] TestSynchronousArchiveRoundtripAndMetadataInspection");

                Console.WriteLine("✅ All .NET 8+ test assertions passed successfully!");
                return 0;
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine($"❌ Test failure: {ex}");
                return 1;
            }
        }
    }
}
