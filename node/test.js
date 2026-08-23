// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.

const assert = require('assert');
const path = require('path');
const fs = require('fs');
const ttzip = require('./index');

async function runTests() {
  console.log('⚡️ Running TTZip Node.js SDK Test Suite...');

  // 1. Version Check
  assert.strictEqual(ttzip.version(), '1.0.0', 'Version should be 1.0.0');
  console.log('  [PASS] Version check (1.0.0)');

  // 2. CRC-32 Check
  const buf = Buffer.from('TTZip High-Throughput Node SDK');
  const crc = ttzip.crc32(buf);
  assert(crc > 0, 'CRC32 should be non-zero');
  console.log(`  [PASS] CRC32 computation: 0x${crc.toString(16).toUpperCase()}`);

  // 3. Compression & Extraction Round-Trip
  const tmpDir = path.join(__dirname, 'test_tmp');
  fs.mkdirSync(tmpDir, { recursive: true });

  const sampleFile = path.join(tmpDir, 'sample.txt');
  fs.writeFileSync(sampleFile, 'Hello Node.js from TTZip Pure Engine!');

  const archiveFile = path.join(tmpDir, 'sample.zip');
  const extractDir = path.join(tmpDir, 'extracted');

  await ttzip.compress([sampleFile], archiveFile);
  assert(fs.existsSync(archiveFile), 'Archive file should exist');
  console.log('  [PASS] Archive creation');

  await ttzip.extract(archiveFile, extractDir);
  const extractedFile = path.join(extractDir, 'sample.txt');
  assert(fs.existsSync(extractedFile), 'Extracted file should exist');
  const content = fs.readFileSync(extractedFile, 'utf8');
  assert.strictEqual(content, 'Hello Node.js from TTZip Pure Engine!');
  console.log('  [PASS] Archive extraction & payload verification');

  // Clean up
  fs.rmSync(tmpDir, { recursive: true, force: true });

  console.log('✅ All Node.js SDK tests passed successfully!');
}

runTests().catch(err => {
  console.error('❌ Node.js test failure:', err);
  process.exit(1);
});
