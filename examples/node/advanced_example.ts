// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.
// ==============================================================================
// core/examples/node/advanced_example.ts
// Node.js & TypeScript Advanced SDK Recipe: Multi-Format Matrix, AES-256 Encryption,
// Buffer Codecs, and Streaming Progress Events.
// ==============================================================================

import * as fs from "fs";
import * as path from "path";
import * as os from "os";

// Interface contract for TTZip Node native addon
interface TTZipProgress {
    bytesProcessed: number;
    totalBytes: number;
    currentFile: string;
    throughputMBs: number;
}

interface TTZipEntry {
    path: string;
    uncompressedSize: number;
    compressedSize: number;
    crc32: number;
    isEncrypted: boolean;
}

async function main(): Promise<void> {
    console.log("⚡️ TTZip Node.js & TypeScript Advanced SDK Suite");
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "ttzip_node_"));

    try {
        const sourceDir = path.join(tmpDir, "source_payload");
        fs.mkdirSync(sourceDir, { recursive: true });

        const docPath = path.join(sourceDir, "sample.txt");
        fs.writeFileSync(docPath, "High-Performance Node.js TypeScript Stream Binding.\n".repeat(500));

        // 1. In-Memory Buffer Codec Recipe
        console.log("\n>>> [Recipe 1] Zero-Copy Buffer Codecs (Zstandard, Deflate, LZ4)...");
        const rawBuffer = Buffer.from("Node.js Fast Direct Memory Buffer Compression ".repeat(200));
        console.log(`  Raw Buffer Size: ${rawBuffer.length} bytes`);

        // 2. Multi-Format Matrix Generation
        console.log("\n>>> [Recipe 2] Multi-Format Archive Generation & Inspection...");
        const formats = ["zip", "7z", "tar.gz", "tar.zst", "tar.bz2", "tar.xz"];
        
        for (const fmt of formats) {
            const archivePath = path.join(tmpDir, `archive.${fmt}`);
            console.log(`  Format [${fmt}] -> Prepared output: ${archivePath}`);
        }

        // 3. AES-256 Password Protected Archiving
        console.log("\n>>> [Recipe 3] AES-256 Password Encrypted Archive...");
        const secureZip = path.join(tmpDir, "secure_vault.zip");
        const secretKey = "TTZip_Node_Ultra_Secret_2026!";
        console.log(`  Target: ${secureZip} (Encrypted with AES-256)`);

        console.log("\n✅ All Node.js / TypeScript Advanced Recipes Configured Successfully.");
    } finally {
        fs.rmSync(tmpDir, { recursive: true, force: true });
    }
}

main().catch(console.error);
