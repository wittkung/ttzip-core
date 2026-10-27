# 🔷 TTZip .NET 8+ (C#) Developer Guide

[![NuGet](https://img.shields.io/badge/nuget-v1.0.0-blue.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/sdk/dotnet/src/TTZip/TTZip.csproj)
[![.NET 8+](https://img.shields.io/badge/.NET-8.0%2B%20%7C%20C%23%2012-purple.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/sdk/dotnet/src/TTZip/TTZip.csproj)
[![Zero-Copy](https://img.shields.io/badge/Memory-ReadOnlySpan%20%26%20SafeHandle-brightgreen.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/sdk/dotnet/src/TTZip/TTZip.cs#L215)

`TTZip` is the official high-performance .NET 8+ SDK for TTZip. It utilizes modern C# features including **`ReadOnlySpan<byte>` zero-copy buffer slicing**, `SafeHandle` memory lifecycle management, and **`IAsyncEnumerable<ArchiveProgress>` reactive channels** for non-blocking server and desktop workloads.

---

## 1. Installation & Runtime Resolution

Install the NuGet package:

```bash
dotnet add package TTZip --version 1.0.0
```

### Cross-Platform RID Native Asset Support

TTZip ships precompiled native runtime binaries for all major platforms:
- **macOS Apple Silicon**: `runtimes/osx-arm64/native/libttzip_engine.dylib`
- **macOS Intel**: `runtimes/osx-x64/native/libttzip_engine.dylib`
- **Linux x64 / ARM64**: `runtimes/linux-x64/native/libttzip_engine.so`
- **Windows x64 / ARM64**: `runtimes/win-x64/native/ttzip_engine.dll`

The built-in `NativeResolver` automatically locates and loads the appropriate library at runtime via `NativeLibrary.SetDllImportResolver`.

---

## 2. Quickstart Code Examples

### 2.1 Reactive Async Compression (`IAsyncEnumerable`)

Compress files while streaming progress updates via `await foreach`:

```csharp
using System;
using System.Threading;
using System.Threading.Tasks;
using TTZip;

class Program
{
    static async Task Main(string[] args)
    {
        string[] sources = new[] {
            "C:\\data\\reports",
            "C:\\data\\database.bak"
        };
        string destination = "C:\\dist\\backup_2026.7z";

        using var cts = new CancellationTokenSource(TimeSpan.FromMinutes(5));

        Console.WriteLine("Starting TTZip asynchronous multi-threaded compression...");

        await foreach (var progress in TTZipEngine.CreateArchiveAsync(
            sources,
            destination,
            format: ArchiveFormat.SevenZip,
            level: CompressionLevel.Normal, // Level 6
            password: "SecretPassword2026!",
            threads: 0, // Auto-detect CPU cores
            cancellationToken: cts.Token
        ))
        {
            Console.WriteLine($"[{progress.FractionCompleted * 100:F1}%] Processing: {progress.CurrentEntryPath}");
        }

        Console.WriteLine($"Archive created successfully at: {destination}");
    }
}
```

### 2.2 Safe Archive Extraction (Zip-Slip Immune)

```csharp
using System;
using System.Threading.Tasks;
using TTZip;

class Extractor
{
    public static async Task ExtractBundleAsync(string archivePath, string outputDir)
    {
        Console.WriteLine($"Extracting {archivePath} to {outputDir}...");

        await foreach (var progress in TTZipEngine.ExtractArchiveAsync(
            archivePath,
            outputDir,
            password: "SecretPassword2026!"
        ))
        {
            Console.WriteLine($"Extracted: {progress.CurrentEntryPath} ({progress.ProcessedBytes} bytes)");
        }

        Console.WriteLine("Extraction complete.");
    }
}
```

### 2.3 Inspecting Archive Metadata

```csharp
using System;
using TTZip;

class Inspector
{
    public static void ListEntries(string archivePath)
    {
        var entries = TTZipEngine.InspectArchive(archivePath, password: "SecretPassword2026!");

        Console.WriteLine($"Archive contains {entries.Count} entries:");
        foreach (var entry in entries)
        {
            Console.WriteLine($"  - {entry.Path,-35} | {entry.UncompressedSize,12:N0} B | CRC32: {entry.Crc32:X8} | Dir: {entry.IsDirectory}");
        }
    }
}
```

---

## 3. High-Speed SIMD Checksums on `ReadOnlySpan<byte>`

Compute hardware-accelerated CRC-32 (>40 GB/s on Apple Silicon / AVX-512) and CRC-64 directly on stack or heap spans without memory allocations:

```csharp
using System;
using System.Text;
using TTZip;

class ChecksumDemo
{
    public static void Main()
    {
        ReadOnlySpan<byte> data = Encoding.UTF8.GetBytes("High-Throughput .NET 8 Span-Based Checksum Payload");

        uint crc32 = TTZipEngine.ComputeCrc32(data);
        ulong crc64 = TTZipEngine.ComputeCrc64(data);

        Console.WriteLine($"SIMD CRC-32: 0x{crc32:X8}");
        Console.WriteLine($"SIMD CRC-64: 0x{crc64:X16}");
        Console.WriteLine($"Engine Version: {TTZipEngine.Version}");
        Console.WriteLine($"Hardware Acceleration Active: {TTZipEngine.IsHardwareAccelerated}");
    }
}
```

---

## 4. ASP.NET Core File Streaming Controller Recipe

Integrate TTZip into an ASP.NET Core Web API for on-the-fly streaming archive creation:

```csharp
using Microsoft.AspNetCore.Mvc;
using System.IO;
using System.Threading.Tasks;
using TTZip;

[ApiController]
[Route("api/[controller]")]
public class ArchiveController : ControllerBase
{
    [HttpPost("bundle")]
    public async Task<IActionResult> CreateBundle([FromBody] string[] filePaths)
    {
        string tempArchive = Path.Combine(Path.GetTempPath(), $"{Guid.NewGuid()}.zip");

        TTZipEngine.CreateArchive(
            filePaths,
            tempArchive,
            format: ArchiveFormat.Zip,
            level: CompressionLevel.Fast
        );

        var fileStream = new FileStream(tempArchive, FileMode.Open, FileAccess.Read, FileShare.Read, 4096, FileOptions.DeleteOnClose);
        return File(fileStream, "application/zip", "download.zip");
    }
}
```
