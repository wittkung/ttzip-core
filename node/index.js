// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for Node.js.

const path = require('path');
const fs = require('fs');

function loadNativeBinding() {
  const candidates = [
    path.join(__dirname, 'ttzip.node'),
    path.join(__dirname, '..', 'rust', 'target', 'release', 'libttzip_node.dylib'),
    path.join(__dirname, '..', 'rust', 'target', 'release', 'libttzip_node.so'),
    path.join(__dirname, '..', 'rust', 'target', 'release', 'ttzip_node.dll'),
    path.join(__dirname, '..', 'rust', 'target', 'debug', 'libttzip_node.dylib'),
    path.join(__dirname, '..', 'rust', 'target', 'debug', 'libttzip_node.so'),
  ];

  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) {
      try {
        return require(candidate);
      } catch (e) {
        // Continue searching candidates
      }
    }
  }
  return null;
}

const native = loadNativeBinding();

function version() {
  if (native && native.version) return native.version();
  return "1.0.0";
}

function isHardwareAccelerated() {
  if (native && native.isHardwareAccelerated) return native.isHardwareAccelerated();
  return false;
}

function crc32(buffer, seed) {
  if (!Buffer.isBuffer(buffer)) {
    throw new TypeError('Expected a Buffer');
  }
  if (native && native.crc32) {
    return native.crc32(buffer, seed);
  }
  let crc = 0 ^ (-1);
  for (let i = 0; i < buffer.length; i++) {
    crc = (crc >>> 8) ^ crc32Table[(crc ^ buffer[i]) & 0xFF];
  }
  return (crc ^ (-1)) >>> 0;
}

const crc32Table = (() => {
  let c;
  const table = [];
  for (let n = 0; n < 256; n++) {
    c = n;
    for (let k = 0; k < 8; k++) {
      c = ((c & 1) ? (0xEDB88320 ^ (c >>> 1)) : (c >>> 1));
    }
    table[n] = c;
  }
  return table;
})();

function crc64(buffer, seed) {
  if (!Buffer.isBuffer(buffer)) {
    throw new TypeError('Expected a Buffer');
  }
  if (native && native.crc64) {
    return native.crc64(buffer, seed);
  }
  throw new Error('CRC64 requires native N-API module');
}

function compressBuffer(buffer, format, level) {
  if (!Buffer.isBuffer(buffer)) {
    throw new TypeError('Expected a Buffer');
  }
  if (native && native.compressBuffer) {
    return native.compressBuffer(buffer, format, level);
  }
  throw new Error('compressBuffer requires native N-API module');
}

function decompressBuffer(buffer, format) {
  if (!Buffer.isBuffer(buffer)) {
    throw new TypeError('Expected a Buffer');
  }
  if (native && native.decompressBuffer) {
    return native.decompressBuffer(buffer, format);
  }
  throw new Error('decompressBuffer requires native N-API module');
}

function decompressInto(compressed, target, format) {
  if (!Buffer.isBuffer(compressed) || !Buffer.isBuffer(target)) {
    throw new TypeError('Expected Buffer arguments');
  }
  if (native && native.decompressInto) {
    return native.decompressInto(compressed, target, format);
  }
  throw new Error('decompressInto requires native N-API module');
}

const { execFile } = require('child_process');
const util = require('util');
const execFileAsync = util.promisify(execFile);

function findCliBinary() {
  const candidates = [
    path.join(__dirname, '..', 'bin', 'ttzip'),
    path.join(__dirname, '..', 'rust', 'target', 'release', 'ttzip'),
    path.join(__dirname, '..', 'rust', 'target', 'debug', 'ttzip'),
  ];
  for (const c of candidates) {
    if (fs.existsSync(c)) return c;
  }
  return 'ttzip';
}

async function compress(inputs, destination, options = {}) {
  if (native && native.compress) {
    return native.compress(inputs, destination, options);
  }
  const cli = findCliBinary();
  const args = ['create', destination, ...inputs];
  if (options.password) {
    args.push('--password', options.password);
  }
  if (options.level !== undefined) {
    args.push('--level', String(options.level));
  }
  await execFileAsync(cli, args);
}

async function extract(archivePath, destination, options = {}) {
  if (native && native.extract) {
    return native.extract(archivePath, destination, options);
  }
  const cli = findCliBinary();
  const args = ['extract', archivePath, '-o', destination];
  if (options.password) {
    args.push('--password', options.password);
  }
  await execFileAsync(cli, args);
}

async function inspect(archivePath, password) {
  if (native && native.inspect) {
    return native.inspect(archivePath, password);
  }
  const cli = findCliBinary();
  const args = ['list', archivePath];
  if (password) {
    args.push('--password', password);
  }
  const { stdout } = await execFileAsync(cli, args);
  return stdout;
}

module.exports = {
  version,
  isHardwareAccelerated,
  crc32,
  crc64,
  compressBuffer,
  decompressBuffer,
  decompressInto,
  compress,
  extract,
  inspect,
};
