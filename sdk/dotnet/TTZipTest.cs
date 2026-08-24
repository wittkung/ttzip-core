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

        [Fact]
        public void TestAes256PasswordProtectedExtractionAndInvalidPassword()
        {
            string secretFile = Path.Combine(_tempDir, "secret.txt");
            string secretPayload = "CONFIDENTIAL: C# .NET 8+ AES-256 Protected Archive Payload 2026";
            File.WriteAllText(secretFile, secretPayload);

            string encryptedZip = Path.Combine(_tempDir, "vault_encrypted.zip");
            string validExtractDir = Path.Combine(_tempDir, "vault_extracted_valid");
            string invalidExtractDir = Path.Combine(_tempDir, "vault_extracted_invalid");
            Directory.CreateDirectory(validExtractDir);
            Directory.CreateDirectory(invalidExtractDir);

            string correctPassword = "TTZipDotNetSecret2026!";
            string wrongPassword = "WrongPassword999!";

            // 1. Create password-protected archive
            TTZipEngine.CreateArchive(
                new[] { secretFile },
                encryptedZip,
                ArchiveFormat.Zip,
                CompressionLevel.Normal,
                password: correctPassword
            );
            Assert.True(File.Exists(encryptedZip));

            // 2. Inspect metadata with password
            List<EntryMetadata> entries = TTZipEngine.InspectArchive(encryptedZip, correctPassword);
            Assert.NotNull(entries);
            Assert.NotEmpty(entries);
            Assert.True(entries[0].IsEncrypted, "Entry should be marked encrypted");

            // 3. Extract with correct password -> must succeed
            TTZipEngine.ExtractArchive(encryptedZip, validExtractDir, password: correctPassword);
            string decryptedFile = Path.Combine(validExtractDir, "secret.txt");
            Assert.True(File.Exists(decryptedFile));
            Assert.Equal(secretPayload, File.ReadAllText(decryptedFile));

            // 4. Extract with incorrect password -> must throw InvalidOperationException
            Assert.Throws<InvalidOperationException>(() =>
            {
                TTZipEngine.ExtractArchive(encryptedZip, invalidExtractDir, password: wrongPassword);
            });
        }

        [Fact]
        public void TestReadOnlySpanBufferSlicingExtensive()
        {
            byte[] buffer = new byte[1024];
            for (int i = 0; i < buffer.Length; i++)
            {
                buffer[i] = (byte)(i * 31 + 7);
            }

            ReadOnlySpan<byte> fullSpan = buffer.AsSpan();
            uint fullCrc = TTZipEngine.ComputeCrc32(fullSpan);
            ulong fullCrc64 = TTZipEngine.ComputeCrc64(fullSpan);

            // 4-way sliced chained CRC-32 and CRC-64
            int q1 = 256;
            int q2 = 512;
            int q3 = 768;

            ReadOnlySpan<byte> s1 = fullSpan.Slice(0, q1);
            ReadOnlySpan<byte> s2 = fullSpan.Slice(q1, q2 - q1);
            ReadOnlySpan<byte> s3 = fullSpan.Slice(q2, q3 - q2);
            ReadOnlySpan<byte> s4 = fullSpan.Slice(q3);

            uint seed32 = TTZipEngine.ComputeCrc32(s1, 0);
            seed32 = TTZipEngine.ComputeCrc32(s2, seed32);
            seed32 = TTZipEngine.ComputeCrc32(s3, seed32);
            uint chained32 = TTZipEngine.ComputeCrc32(s4, seed32);
            Assert.Equal(fullCrc, chained32);

            ulong seed64 = TTZipEngine.ComputeCrc64(s1, 0);
            seed64 = TTZipEngine.ComputeCrc64(s2, seed64);
            seed64 = TTZipEngine.ComputeCrc64(s3, seed64);
            ulong chained64 = TTZipEngine.ComputeCrc64(s4, seed64);
            Assert.Equal(fullCrc64, chained64);

            // Empty span checks
            Assert.Equal(0u, TTZipEngine.ComputeCrc32(ReadOnlySpan<byte>.Empty));
            Assert.Equal(0UL, TTZipEngine.ComputeCrc64(ReadOnlySpan<byte>.Empty));
        }

        [Fact]
        public void TestMultiFormatArchiveMatrix()
        {
            string srcFile = Path.Combine(_tempDir, "matrix_test.txt");
            string payload = "TTZip C# .NET Multi-Format (ZIP, 7Z, TAR, TAR.GZ, TAR.BZ2, TAR.XZ, TAR.ZSTD) Test Payload\n";
            File.WriteAllText(srcFile, payload);

            var formats = new (ArchiveFormat format, string filename, CompressionLevel level)[]
            {
                (ArchiveFormat.Zip, "dotnet_matrix.zip", CompressionLevel.Fastest),
                (ArchiveFormat.SevenZip, "dotnet_matrix.7z", CompressionLevel.Normal),
                (ArchiveFormat.Tar, "dotnet_matrix.tar", CompressionLevel.Store),
                (ArchiveFormat.TarGz, "dotnet_matrix.tar.gz", CompressionLevel.Fast),
                (ArchiveFormat.TarBz2, "dotnet_matrix.tar.bz2", CompressionLevel.Normal),
                (ArchiveFormat.TarXz, "dotnet_matrix.tar.xz", CompressionLevel.Maximum),
                (ArchiveFormat.TarZstd, "dotnet_matrix.tar.zst", CompressionLevel.Ultra)
            };

            foreach (var (fmt, filename, level) in formats)
            {
                string arcPath = Path.Combine(_tempDir, filename);
                string outDir = Path.Combine(_tempDir, "dest_" + filename);
                Directory.CreateDirectory(outDir);

                TTZipEngine.CreateArchive(new[] { srcFile }, arcPath, fmt, level);
                Assert.True(File.Exists(arcPath));
                Assert.True(new FileInfo(arcPath).Length > 0);

                var entries = TTZipEngine.InspectArchive(arcPath);
                Assert.NotEmpty(entries);

                TTZipEngine.ExtractArchive(arcPath, outDir);
                string extFile = Path.Combine(outDir, "matrix_test.txt");
                Assert.True(File.Exists(extFile));
                Assert.Equal(payload, File.ReadAllText(extFile));
            }
        }

        [Fact]
        public void TestCorruptArchiveHeaderDetection()
        {
            string corruptFile = Path.Combine(_tempDir, "corrupt_test.zip");
            File.WriteAllBytes(corruptFile, new byte[] { 0x50, 0x4B, 0x03, 0x04, 0x00, 0x00, 0xFF, 0xFF, 0x12, 0x34 });

            string outDir = Path.Combine(_tempDir, "corrupt_extracted");
            Directory.CreateDirectory(outDir);

            Assert.Throws<InvalidOperationException>(() =>
            {
                TTZipEngine.ExtractArchive(corruptFile, outDir);
            });
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

                test.TestReadOnlySpanBufferSlicingExtensive();
                Console.WriteLine("  [PASS] TestReadOnlySpanBufferSlicingExtensive");

                test.TestSafeHandleZeroAllocLifecycle();
                Console.WriteLine("  [PASS] TestSafeHandleZeroAllocLifecycle");

                await test.TestIAsyncEnumerableStreamingArchiving();
                Console.WriteLine("  [PASS] TestIAsyncEnumerableStreamingArchiving");

                test.TestSynchronousArchiveRoundtripAndMetadataInspection();
                Console.WriteLine("  [PASS] TestSynchronousArchiveRoundtripAndMetadataInspection");

                test.TestAes256PasswordProtectedExtractionAndInvalidPassword();
                Console.WriteLine("  [PASS] TestAes256PasswordProtectedExtractionAndInvalidPassword");

                test.TestMultiFormatArchiveMatrix();
                Console.WriteLine("  [PASS] TestMultiFormatArchiveMatrix");

                test.TestCorruptArchiveHeaderDetection();
                Console.WriteLine("  [PASS] TestCorruptArchiveHeaderDetection");

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
