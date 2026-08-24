// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

// Swift 6 Actor-based Concurrency Living Example demonstrating:
// - TTZipEngine Swift 6 Actor thread-safe coordination
// - AsyncStream<ArchiveProgress> real-time telemetry
// - AES-256 Encryption & Password Vault Protection
// - Archive Extraction & Integrity Verification
// - Direct & Streaming Operations

import Foundation
import TTZipCore

@main
struct TTZipSwiftExample {
    static func main() async {
        print("========================================================================")
        print("⚡️ TTZip Swift 6 Complete Concurrency & Actor Living Example")
        print("========================================================================")

        let fileManager = FileManager.default
        let tempDir = fileManager.temporaryDirectory.appendingPathComponent("ttzip_swift_example_\(UUID().uuidString)")
        
        do {
            try fileManager.createDirectory(at: tempDir, withIntermediateDirectories: true)
            defer {
                try? fileManager.removeItem(at: tempDir)
            }

            // 1. Prepare sample files
            let sampleFile1 = tempDir.appendingPathComponent("document.json")
            let sampleFile2 = tempDir.appendingPathComponent("report.csv")
            let sampleContent1 = """
            {
                "engine": "TTZipCore",
                "language": "Swift 6",
                "concurrency": "Strict Concurrency / Complete Concurrency",
                "version": "1.0.0"
            }
            """
            let sampleContent2 = (0..<200).map { "row_\($0),Swift6Actor,ProgressTelemetry,100.0\n" }.joined()
            
            try sampleContent1.write(to: sampleFile1, atomically: true, encoding: .utf8)
            try sampleContent2.write(to: sampleFile2, atomically: true, encoding: .utf8)

            let engine = TTZipEngine.shared

            // -----------------------------------------------------------------
            // Section 1: AsyncStream<ArchiveProgress> Real-Time Compression
            // -----------------------------------------------------------------
            print("\n" + String(repeating: "=", count: 72))
            print("⚡ 1. Swift 6 Actor Compression with AsyncStream<ArchiveProgress>")
            print(String(repeating: "=", count: 72))

            let zipOutput = tempDir.appendingPathComponent("stream_output.zip").path
            let inputs = [sampleFile1.path, sampleFile2.path]

            print("• Compressing inputs into: \(zipOutput)")
            let (compressStream, compressTask) = await engine.compress(
                inputs: inputs,
                outputPath: zipOutput,
                format: .zip,
                level: .normal
            )

            // Consume progress stream asynchronously
            let progressObserver = Task {
                for await progress in compressStream {
                    let percent = Int(progress.fractionCompleted * 100)
                    print("  [Progress Stream] \(percent)% | \(progress.bytesProcessed)/\(progress.totalBytes) bytes | Throughput: \(String(format: "%.1f", progress.throughputMBs)) MB/s | Item: \(progress.currentFileName)")
                }
            }

            let compressResult = try await compressTask.value
            _ = await progressObserver.result
            print("• Compression Completed in \(String(format: "%.3f", compressResult.durationSeconds))s: \(compressResult.compressedBytes) bytes created")

            // -----------------------------------------------------------------
            // Section 2: AES-256 Password-Protected Archive
            // -----------------------------------------------------------------
            print("\n" + String(repeating: "=", count: 72))
            print("⚡ 2. AES-256 Encrypted Archive Creation & Extraction")
            print(String(repeating: "=", count: 72))

            let encryptedZipOutput = tempDir.appendingPathComponent("vault_secure.zip").path
            let vaultPassword = "Swift6-Actor-SecurePassword-2026!"

            print("• Creating AES-256 Encrypted Archive: \(encryptedZipOutput)")
            let directResult = try await engine.compressDirect(
                inputs: inputs,
                outputPath: encryptedZipOutput,
                format: .zip,
                level: .normal,
                password: vaultPassword
            )
            print("• Encrypted Archive Created: \(directResult.compressedBytes) bytes")

            // -----------------------------------------------------------------
            // Section 3: Archive Inspection & Encryption Probing
            // -----------------------------------------------------------------
            print("\n" + String(repeating: "=", count: 72))
            print("⚡ 3. Archive Inspection & Encryption Tier Classification")
            print(String(repeating: "=", count: 72))

            let archiveHandle = try await engine.open(at: encryptedZipOutput, password: vaultPassword)
            print("• Probed Archive Format: \(archiveHandle.format.displayName)")
            print("• Probed Encryption Tier: \(archiveHandle.encryptionTier.rawValue)")

            // -----------------------------------------------------------------
            // Section 4: AsyncStream<ArchiveProgress> Extraction
            // -----------------------------------------------------------------
            print("\n" + String(repeating: "=", count: 72))
            print("⚡ 4. Asynchronous Extraction with Telemetry Stream")
            print(String(repeating: "=", count: 72))

            let extractDir = tempDir.appendingPathComponent("extracted_files").path
            print("• Extracting archive to: \(extractDir)")

            let (extractStream, extractTask) = await engine.extract(
                archivePath: encryptedZipOutput,
                destinationDir: extractDir,
                password: vaultPassword
            )

            let extractProgressObserver = Task {
                for await progress in extractStream {
                    let percent = Int(progress.fractionCompleted * 100)
                    print("  [Extract Stream] \(percent)% | \(progress.bytesProcessed) bytes | Item: \(progress.currentFileName)")
                }
            }

            let extractResult = try await extractTask.value
            _ = await extractProgressObserver.result
            print("• Extraction Completed in \(String(format: "%.3f", extractResult.durationSeconds))s into: \(extractResult.destinationDir)")

            // Verify extracted content
            let extractedFile1 = tempDir.appendingPathComponent("extracted_files").appendingPathComponent("document.json")
            if fileManager.fileExists(atPath: extractedFile1.path) {
                let content = try String(contentsOf: extractedFile1, encoding: .utf8)
                print("• Verified Extracted File 1: \(content.contains("TTZipCore") ? "PASS" : "FAIL")")
            }

            print("\n" + String(repeating: "=", count: 72))
            print("🎉 All TTZip Swift 6 Living Example Demonstrations Succeeded!")
            print(String(repeating: "=", count: 72) + "\n")

        } catch {
            print("❌ Error during Swift 6 Example execution: \(error)")
            exit(1)
        }
    }
}
