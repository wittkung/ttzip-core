// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for .NET 8+.
// Standalone runnable quickstart example.

using System;
using System.IO;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using TTZip;

namespace TTZip.Examples
{
    public static class Program
    {
        public static async Task Main(string[] args)
        {
            Console.WriteLine($"⚡️ TTZip .NET 8+ C# SDK Quickstart (v{TTZipEngine.Version})");
            Console.WriteLine($"Hardware SIMD Acceleration: {TTZipEngine.IsHardwareAccelerated}");

            // 1. ReadOnlySpan Zero-Copy SIMD Checksums
            byte[] payload = Encoding.UTF8.GetBytes(".NET 8/9 High-Throughput Archiving & Compression Pipeline 2026");
            ReadOnlySpan<byte> span = payload.AsSpan();

            uint crc32 = TTZipEngine.ComputeCrc32(span);
            ulong crc64 = TTZipEngine.ComputeCrc64(span);
            Console.WriteLine($"SIMD CRC-32: 0x{crc32:X8}");
            Console.WriteLine($"SIMD CRC-64: 0x{crc64:X16}");

            // 2. Setup temporary demo workspace
            string tempDir = Path.Combine(Path.GetTempPath(), "ttzip_dotnet_quickstart_" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(tempDir);

            try
            {
                string file1 = Path.Combine(tempDir, "service_config.json");
                string file2 = Path.Combine(tempDir, "metrics.log");

                await File.WriteAllTextAsync(file1, "{\"runtime\": \".NET 8\", \"engine\": \"TTZip\", \"zero_copy\": true}");
                await File.WriteAllTextAsync(file2, "High-performance unmanaged native archive streaming payload.");

                string syncZip = Path.Combine(tempDir, "sync_demo.zip");
                string asyncZip = Path.Combine(tempDir, "async_stream_demo.zip");
                string extractDir = Path.Combine(tempDir, "extracted");
                Directory.CreateDirectory(extractDir);

                // 3. Synchronous Archive Creation
                Console.WriteLine("\n[1] Creating archive synchronously...");
                TTZipEngine.CreateArchive(
                    new[] { file1, file2 },
                    syncZip,
                    ArchiveFormat.Zip,
                    CompressionLevel.Normal
                );
                Console.WriteLine($"Created: {syncZip} ({new FileInfo(syncZip).Length} bytes)");

                // 4. Archive Inspection
                Console.WriteLine("\n[2] Inspecting archive entry metadata:");
                var entries = TTZipEngine.InspectArchive(syncZip);
                foreach (var entry in entries)
                {
                    Console.WriteLine($"  - {entry.Path} (Uncompressed: {entry.UncompressedSize} B, Compressed: {entry.CompressedSize} B, CRC: 0x{entry.Crc32:X8})");
                }

                // 5. Asynchronous IAsyncEnumerable Progress Streaming
                Console.WriteLine("\n[3] Creating archive with IAsyncEnumerable real-time progress streaming:");
                using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(15));
                await foreach (var progress in TTZipEngine.CreateArchiveAsync(
                    new[] { file1, file2 },
                    asyncZip,
                    ArchiveFormat.Zip,
                    CompressionLevel.Fast,
                    threads: 2,
                    cancellationToken: cts.Token
                ))
                {
                    Console.WriteLine($"  Progress: {progress.FractionCompleted * 100:F1}% ({progress.ProcessedBytes}/{progress.TotalBytes} bytes) - {progress.CurrentEntryPath}");
                }

                // 6. Extraction
                Console.WriteLine("\n[4] Extracting archive...");
                TTZipEngine.ExtractArchive(syncZip, extractDir);
                Console.WriteLine($"Extracted to: {extractDir}");

                string extractedConfig = Path.Combine(extractDir, "service_config.json");
                if (File.Exists(extractedConfig))
                {
                    Console.WriteLine($"Verified payload content: {await File.ReadAllTextAsync(extractedConfig)}");
                }

                Console.WriteLine("\n✅ TTZip .NET 8+ Quickstart completed successfully.");
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
