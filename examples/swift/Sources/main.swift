// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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

            let inputs = try prepareSampleFiles(in: tempDir)
            let engine = TTZipEngine.shared

            try await runStreamCompressionSection(engine: engine, inputs: inputs, tempDir: tempDir)
            let (encryptedZipOutput, vaultPassword) = try await runEncryptedCreationSection(
                engine: engine,
                inputs: inputs,
                tempDir: tempDir
            )
            try await runInspectionAndExtractionSection(
                engine: engine,
                encryptedZipOutput: encryptedZipOutput,
                vaultPassword: vaultPassword,
                tempDir: tempDir,
                fileManager: fileManager
            )

            print("\n" + String(repeating: "=", count: 72))
            print("🎉 All TTZip Swift 6 Living Example Demonstrations Succeeded!")
            print(String(repeating: "=", count: 72) + "\n")
        } catch {
            print("❌ Error during Swift 6 Example execution: \(error)")
            exit(1)
        }
    }

    private static func prepareSampleFiles(in tempDir: URL) throws -> [String] {
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
        return [sampleFile1.path, sampleFile2.path]
    }

    private static func runStreamCompressionSection(
        engine: TTZipEngine,
        inputs: [String],
        tempDir: URL
    ) async throws {
        print("\n" + String(repeating: "=", count: 72))
        print("⚡ 1. Swift 6 Actor Compression with AsyncStream<ArchiveProgress>")
        print(String(repeating: "=", count: 72))

        let zipOutput = tempDir.appendingPathComponent("stream_output.zip").path
        print("• Compressing inputs into: \(zipOutput)")
        let (compressStream, compressTask) = await engine.compress(
            inputs: inputs,
            outputPath: zipOutput,
            format: .zip,
            level: .normal
        )

        let progressObserver = Task {
            for await progress in compressStream {
                let percent = Int(progress.fractionCompleted * 100)
                print("  [Progress Stream] \(percent)% | \(progress.bytesProcessed)/\(progress.totalBytes) bytes | Throughput: \(String(format: "%.1f", progress.throughputMBs)) MB/s | Item: \(progress.currentFileName)")
            }
        }

        let compressResult = try await compressTask.value
        _ = await progressObserver.result
        print("• Compression Completed in \(String(format: "%.3f", compressResult.durationSeconds))s: \(compressResult.compressedBytes) bytes created")
    }

    private static func runEncryptedCreationSection(
        engine: TTZipEngine,
        inputs: [String],
        tempDir: URL
    ) async throws -> (outputPath: String, password: String) {
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
        return (encryptedZipOutput, vaultPassword)
    }

    private static func runInspectionAndExtractionSection(
        engine: TTZipEngine,
        encryptedZipOutput: String,
        vaultPassword: String,
        tempDir: URL,
        fileManager: FileManager
    ) async throws {
        print("\n" + String(repeating: "=", count: 72))
        print("⚡ 3. Archive Inspection & Encryption Tier Classification")
        print(String(repeating: "=", count: 72))

        let archiveHandle = try await engine.open(at: encryptedZipOutput, password: vaultPassword)
        print("• Probed Archive Format: \(archiveHandle.format.displayName)")
        print("• Probed Encryption Tier: \(archiveHandle.encryptionTier.rawValue)")

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

        let extractedFile1 = tempDir.appendingPathComponent("extracted_files").appendingPathComponent("document.json")
        if fileManager.fileExists(atPath: extractedFile1.path) {
            let content = try String(contentsOf: extractedFile1, encoding: .utf8)
            print("• Verified Extracted File 1: \(content.contains("TTZipCore") ? "PASS" : "FAIL")")
        }
    }
}
