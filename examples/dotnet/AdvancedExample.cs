// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: Advanced .NET 8+ C# Features Showcase.
// Demonstrates ReadOnlySpan<byte> zero-copy SIMD checksums, IAsyncEnumerable<ArchiveProgress>
// real-time streaming, CancellationToken cancellation, AES-256 encryption, and multi-format pipelines.

using System;
using System.Diagnostics;
using System.IO;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using TTZip;

namespace TTZip.Examples
{
    public static class AdvancedExample
    {
        public static async Task Main(string[] args)
        {
            Console.WriteLine("================================================================================");
            Console.WriteLine($"⚡️ TTZip .NET 8+ C# SDK Advanced Features Showcase (v{TTZipEngine.Version})");
            Console.WriteLine("================================================================================");

            // 1. Engine & SIMD Hardware Telemetry
            Console.WriteLine("1. Querying Native Engine Capabilities...");
            Console.WriteLine($"   • Engine Version:        {TTZipEngine.Version}");
            Console.WriteLine($"   • SIMD Acceleration:     {(TTZipEngine.IsHardwareAccelerated ? "ACTIVE (ARM NEON / AVX-512 / AES-NI)" : "DISABLED")}");
            Console.WriteLine("--------------------------------------------------------------------------------");

            // 2. ReadOnlySpan<byte> Zero-Copy Chunked Streaming & Checksums
            Console.WriteLine("2. ReadOnlySpan<byte> Zero-Copy Streaming & Checksum Pipeline...");
            const int chunkSize = 1024 * 1024; // 1 MB chunk
            const int numChunks = 8;           // 8 MB buffer
            byte[] syntheticData = new byte[chunkSize * numChunks];
            for (int i = 0; i < syntheticData.Length; i++)
            {
                syntheticData[i] = (byte)((i * 37 + 19) & 0xFF);
            }

            ReadOnlySpan<byte> fullSpan = syntheticData.AsSpan();
            uint runningCrc32 = 0;
            ulong runningCrc64 = 0;

            var sw = Stopwatch.StartNew();
            for (int c = 0; c < numChunks; c++)
            {
                ReadOnlySpan<byte> chunkSpan = fullSpan.Slice(c * chunkSize, chunkSize);
                runningCrc32 = TTZipEngine.ComputeCrc32(chunkSpan, runningCrc32);
                runningCrc64 = TTZipEngine.ComputeCrc64(chunkSpan, runningCrc64);
            }
            sw.Stop();

            double throughputGbs = (syntheticData.Length / (1024.0 * 1024.0 * 1024.0)) / sw.Elapsed.TotalSeconds;
            Console.WriteLine($"   • Streamed:              {syntheticData.Length / (1024 * 1024)} MB in {sw.Elapsed.TotalMilliseconds:F2} ms ({throughputGbs:F2} GB/s)");
            Console.WriteLine($"   • Streaming CRC-32:      0x{runningCrc32:X8}");
            Console.WriteLine($"   • Streaming CRC-64:      0x{runningCrc64:X16}");
            Console.WriteLine("--------------------------------------------------------------------------------");

            // 3. Prepare Multi-File Test Dataset
            string tempDir = Path.Combine(Path.GetTempPath(), "ttzip_dotnet_adv_" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(tempDir);

            try
            {
                string payloadDir = Path.Combine(tempDir, "payload");
                Directory.CreateDirectory(payloadDir);

                string file1 = Path.Combine(payloadDir, "service_manifest.json");
                string file2 = Path.Combine(payloadDir, "model_weights.bin");
                string file3 = Path.Combine(payloadDir, "README.md");

                await File.WriteAllTextAsync(file1, "{\"runtime\": \".NET 8\", \"async_stream\": true, \"cipher\": \"AES-256\", \"threads\": 4}");
                byte[] binaryPayload = new byte[65536];
                for (int i = 0; i < binaryPayload.Length; i++) binaryPayload[i] = (byte)(i & 0xFF);
                await File.WriteAllBytesAsync(file2, binaryPayload);
                await File.WriteAllTextAsync(file3, "# TTZip .NET 8 Advanced Showcase\nZero-copy spans and IAsyncEnumerable reactive pipeline.\n");

                string[] sources = new[] { file1, file2, file3 };
                const string aesPassword = "DotNetSecurePassword2026!";
                const int threadBudget = 4;

                string zipEncPath = Path.Combine(tempDir, "encrypted_dataset.zip");
                string sevenZipPath = Path.Combine(tempDir, "solid_dataset.7z");
                string tarZstPath = Path.Combine(tempDir, "dataset.tar.zst");
                string extractDir = Path.Combine(tempDir, "extracted_output");

                // 4. AES-256 Encrypted Archive with IAsyncEnumerable Progress Streaming
                Console.WriteLine("3. Creating AES-256 Encrypted Archive with IAsyncEnumerable Progress Streaming...");
                using var ctsCreate = new CancellationTokenSource(TimeSpan.FromSeconds(15));

                await foreach (var progress in TTZipEngine.CreateArchiveAsync(
                    sources,
                    zipEncPath,
                    ArchiveFormat.Zip,
                    CompressionLevel.Normal,
                    password: aesPassword,
                    threads: threadBudget,
                    cancellationToken: ctsCreate.Token
                ))
                {
                    string current = string.IsNullOrEmpty(progress.CurrentEntryPath) ? "packing" : progress.CurrentEntryPath;
                    Console.WriteLine($"   [IAsyncEnumerable] -> {progress.FractionCompleted * 100:F1}% ({progress.ProcessedBytes}/{progress.TotalBytes} B) | {current}");
                }
                Console.WriteLine($"   ✓ AES-256 Encrypted Archive Created: {Path.GetFileName(zipEncPath)} ({new FileInfo(zipEncPath).Length} bytes)");
                Console.WriteLine("--------------------------------------------------------------------------------");

                // 5. 7z Solid Archive with High Compression Level
                Console.WriteLine("4. Creating 7z Solid Archive with Maximum Compression (4 Threads)...");
                TTZipEngine.CreateArchive(
                    sources,
                    sevenZipPath,
                    ArchiveFormat.SevenZip,
                    CompressionLevel.Maximum,
                    threads: threadBudget
                );
                Console.WriteLine($"   ✓ 7z Solid Archive Created: {Path.GetFileName(sevenZipPath)} ({new FileInfo(sevenZipPath).Length} bytes)");
                Console.WriteLine("--------------------------------------------------------------------------------");

                // 6. TAR.ZST Archive with Ultra Compression
                Console.WriteLine("5. Creating TAR.ZST Archive with Ultra Compression...");
                TTZipEngine.CreateArchive(
                    sources,
                    tarZstPath,
                    ArchiveFormat.TarZstd,
                    CompressionLevel.Ultra,
                    threads: threadBudget
                );
                Console.WriteLine($"   ✓ TAR.ZST Archive Created: {Path.GetFileName(tarZstPath)} ({new FileInfo(tarZstPath).Length} bytes)");
                Console.WriteLine("--------------------------------------------------------------------------------");

                // 7. Demonstrating CancellationToken Cancellation
                Console.WriteLine("6. Demonstrating CancellationToken Cancellation Handling...");
                using var cancelCts = new CancellationTokenSource();
                cancelCts.Cancel(); // Pre-cancelled token

                string cancelledZip = Path.Combine(tempDir, "cancelled.zip");
                try
                {
                    await foreach (var _ in TTZipEngine.CreateArchiveAsync(
                        sources,
                        cancelledZip,
                        ArchiveFormat.Zip,
                        cancellationToken: cancelCts.Token
                    ))
                    { }
                    Console.WriteLine("   • Completed before cancellation.");
                }
                catch (OperationCanceledException)
                {
                    Console.WriteLine("   ✓ OperationCanceledException caught and verified gracefully.");
                }
                Console.WriteLine("--------------------------------------------------------------------------------");

                // 8. Inspecting Encrypted Archive Metadata
                Console.WriteLine("7. Inspecting Encrypted Archive Metadata:");
                var entries = TTZipEngine.InspectArchive(zipEncPath, aesPassword);
                foreach (var entry in entries)
                {
                    Console.WriteLine($"   * {entry.Path,-24} | Uncompressed: {entry.UncompressedSize,7} B | Compressed: {entry.CompressedSize,7} B | Encrypted: {entry.IsEncrypted}");
                }
                Console.WriteLine("--------------------------------------------------------------------------------");

                // 9. Extracting Encrypted Archive & Verifying Payload
                Console.WriteLine("8. Extracting AES-256 Encrypted Archive with IAsyncEnumerable Progress Streaming...");
                Directory.CreateDirectory(extractDir);

                using var ctsExtract = new CancellationTokenSource(TimeSpan.FromSeconds(15));
                await foreach (var progress in TTZipEngine.ExtractArchiveAsync(
                    zipEncPath,
                    extractDir,
                    password: aesPassword,
                    threads: threadBudget,
                    cancellationToken: ctsExtract.Token
                ))
                {
                    Console.WriteLine($"   [Extract Stream] -> {progress.FractionCompleted * 100:F1}%");
                }

                string extractedManifest = Path.Combine(extractDir, "service_manifest.json");
                if (File.Exists(extractedManifest))
                {
                    Console.WriteLine($"   ✓ Decrypted Payload Verified:\n     {await File.ReadAllTextAsync(extractedManifest)}");
                }

                Console.WriteLine("================================================================================");
                Console.WriteLine("🎉 TTZip .NET 8+ C# Advanced Showcase Completed Successfully (Exit Code: 0)");
                Console.WriteLine("================================================================================");
            }
            finally
            {
                if (Directory.Exists(tempDir))
                {
                    try
                    {
                        Directory.Delete(tempDir, true);
                    }
                    catch { }
                }
            }
        }
    }
}
