// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import { version, crc32, createArchive, extractArchive, inspectArchive } from "ttzip";

async function main() {
  console.log(`⚡️ TTZip Node.js & TypeScript SDK (v${version()})`);

  const data = Buffer.from("TypeScript High Throughput Compression");
  const checksum = crc32(data);
  console.log(`CRC-32: 0x${checksum.toString(16).toUpperCase()}`);

  await createArchive(["package.json"], "demo.zip", { level: 6 });
  const entries = await inspectArchive("demo.zip");
  console.log(`Archived ${entries.length} entries:`, entries);

  await extractArchive("demo.zip", "extracted_output");
}

main().catch(console.error);
