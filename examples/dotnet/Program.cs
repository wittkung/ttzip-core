// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
using System;
using System.Text;
using TTZip;

namespace TTZip.Examples
{
    class Program
    {
        static void Main(string[] args)
        {
            Console.WriteLine($"⚡️ TTZip C# .NET SDK Example (v{TTZipEngine.GetVersion()})");

            byte[] data = Encoding.UTF8.GetBytes(".NET 8/9 High Performance Data Pipeline");
            uint crc = TTZipEngine.ComputeCrc32(data);
            Console.WriteLine($"CRC-32: 0x{crc:X8}");

            TTZipEngine.CreateArchive(new string[] { "Program.cs" }, "dotnet_demo.zip", CompressionLevel.Normal);
            TTZipEngine.ExtractArchive("dotnet_demo.zip", "extracted_dotnet");
            Console.WriteLine("Archive creation and extraction completed.");
        }
    }
}
