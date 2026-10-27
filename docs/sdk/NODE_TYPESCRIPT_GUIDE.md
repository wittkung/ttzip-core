# 🟢 TTZip Node.js & TypeScript Developer Guide

[![npm](https://img.shields.io/badge/npm-%40ttzip%2Fcore-red.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/node/package.json)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.0%2B%20Strict-blue.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/node/index.d.ts)
[![Node.js](https://img.shields.io/badge/Node.js-18.0%2B-brightgreen.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/node/package.json)

`@ttzip/core` is the official Node.js and TypeScript binding for TTZip. Engineered as a native N-API addon, it provides **async Promise-based archive manipulation**, in-memory `Buffer` codecs, hardware-accelerated CRC-32 checksums, and zero event loop blocking.

---

## 1. Installation & TypeScript Setup

Install the npm package:

```bash
npm install @ttzip/core
# or
pnpm add @ttzip/core
```

### TypeScript Configuration (`tsconfig.json`)

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true
  }
}
```

---

## 2. TypeScript API Reference

The package exports full TypeScript type definitions from [index.d.ts](file:///Users/kevintung/Documents/dev/products/ttzip/core/node/index.d.ts):

```typescript
export interface EntryMetadata {
  path: string;
  uncompressedSize: number;
  compressedSize: number;
  crc32: number;
  isDirectory: boolean;
  isEncrypted: boolean;
}

export interface CompressOptions {
  format?: 'zip' | '7z' | 'tar' | 'tar.gz' | 'tar.zst';
  level?: number;
  password?: string;
  threads?: number;
}

export interface ExtractOptions {
  destination?: string;
  password?: string;
  stripComponents?: number;
}
```

---

## 3. Quickstart Code Examples

### 3.1 Asynchronous Archive Creation (ZIP / 7z)

```typescript
import { compress, CompressOptions } from '@ttzip/core';
import path from 'path';

async function createBackup() {
  const sources = [
    path.resolve('./src'),
    path.resolve('./package.json')
  ];
  const destination = path.resolve('./dist/project_backup.7z');

  const options: CompressOptions = {
    format: '7z',
    level: 6, // Normal compression
    password: 'SecretPassword2026!',
    threads: 0 // Auto-detect CPU cores
  };

  try {
    console.log('Compressing files asynchronously with TTZip...');
    await compress(sources, destination, options);
    console.log(`Archive successfully created at: ${destination}`);
  } catch (err) {
    console.error('Compression failed:', err);
  }
}

createBackup();
```

### 3.2 Safe Archive Extraction (Zip-Slip Immune)

Extract archives safely with built-in path canonicalization:

```typescript
import { extract } from '@ttzip/core';
import path from 'path';

async function unpackArchive() {
  const archivePath = path.resolve('./dist/project_backup.7z');
  const targetDir = path.resolve('./dist/extracted_output');

  try {
    await extract(archivePath, targetDir, {
      password: 'SecretPassword2026!'
    });
    console.log(`All files safely extracted to: ${targetDir}`);
  } catch (err) {
    console.error('Extraction error:', err);
  }
}

unpackArchive();
```

### 3.3 Non-Extracting Metadata Inspection

Inspect entry headers without touching the disk:

```typescript
import { inspect, EntryMetadata } from '@ttzip/core';

async function listContents() {
  const entries: EntryMetadata[] = await inspect('./dist/project_backup.7z', 'SecretPassword2026!');

  console.log(`Archive contains ${entries.length} entries:`);
  for (const entry of entries) {
    console.log(`  - ${entry.path.padEnd(30)} | ${entry.uncompressedSize.toString().padStart(10)} bytes | CRC32: 0x${entry.crc32.toString(16).toUpperCase()} | Dir: ${entry.isDirectory}`);
  }
}

listContents();
```

---

## 4. In-Memory Buffer Codecs & SIMD Checksums

Perform ultra-fast in-memory buffer transformations:

```typescript
import { compressBuffer, decompressBuffer, crc32, version } from '@ttzip/core';

const rawData = Buffer.from('Apple Silicon SIMD Vectorized Payload for Node.js Services\n'.repeat(500));

// 1. Hardware-accelerated CRC-32 (>40 GB/s)
const checksum = crc32(rawData);
console.log(`CRC-32: 0x${checksum.toString(16).toUpperCase()}`);

// 2. In-Memory DEFLATE / Zstandard compression
const compressedZstd = compressBuffer(rawData, 'zstd', 3);
const decompressedZstd = decompressBuffer(compressedZstd, 'zstd');

console.assert(decompressedZstd.equals(rawData), 'Zstandard roundtrip failed!');
console.log(`Original: ${rawData.length} B | Compressed: ${compressedZstd.length} B`);
console.log(`TTZip Engine Version: ${version()}`);
```

---

## 5. Express.js Dynamic Archive Streaming Recipe

Stream dynamic zip archives directly over HTTP to clients without writing temporary files to disk:

```typescript
import express from 'express';
import { compress } from '@ttzip/core';
import fs from 'fs';
import path from 'path';
import os from 'os';

const app = express();

app.get('/download-export', async (req, res) => {
  const tempZipPath = path.join(os.tmpdir(), `export_${Date.now()}.zip`);
  const filesToInclude = [
    path.resolve('./data/report.pdf'),
    path.resolve('./data/summary.csv')
  ];

  try {
    await compress(filesToInclude, tempZipPath, { format: 'zip', level: 6 });

    res.download(tempZipPath, 'user_export.zip', (err) => {
      // Clean up temporary file after transmission
      fs.unlink(tempZipPath, () => {});
    });
  } catch (err) {
    res.status(500).send('Failed to generate export archive.');
  }
});

app.listen(3000, () => {
  console.log('Server running on http://localhost:3000');
});
```
