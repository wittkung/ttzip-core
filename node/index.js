// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.

const { execFileSync, execFile } = require('child_process');
const path = require('path');
const fs = require('fs');

// Locate ttzip binary
function getBinaryPath() {
  const localBin = path.join(__dirname, '..', 'rust', 'target', 'release', 'ttzip');
  if (fs.existsSync(localBin)) {
    return localBin;
  }
  return 'ttzip';
}

function version() {
  return "1.0.0";
}

function crc32(buffer) {
  if (!Buffer.isBuffer(buffer)) {
    throw new TypeError('Expected a Buffer');
  }
  // Table-driven fast CRC32 fallback / native
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

async function compress(inputs, destination, options = {}) {
  const bin = getBinaryPath();
  const args = ['create', destination, ...inputs];
  if (options.level !== undefined) {
    args.push('-l', String(options.level));
  }
  if (options.password) {
    args.push('-p', options.password);
  }
  return new Promise((resolve, reject) => {
    execFile(bin, args, (err, stdout, stderr) => {
      if (err) return reject(new Error(stderr || err.message));
      resolve();
    });
  });
}

async function extract(archivePath, destination, options = {}) {
  const bin = getBinaryPath();
  const args = ['extract', archivePath, '-o', destination];
  if (options.password) {
    args.push('-p', options.password);
  }
  return new Promise((resolve, reject) => {
    execFile(bin, args, (err, stdout, stderr) => {
      if (err) return reject(new Error(stderr || err.message));
      resolve();
    });
  });
}

async function inspect(archivePath, password) {
  const bin = getBinaryPath();
  const args = ['list', archivePath, '--json'];
  if (password) {
    args.push('-p', password);
  }
  return new Promise((resolve, reject) => {
    execFile(bin, args, (err, stdout, stderr) => {
      if (err) return reject(new Error(stderr || err.message));
      try {
        const lines = stdout.trim().split('\n').filter(Boolean);
        const entries = lines.map(line => JSON.parse(line));
        resolve(entries);
      } catch (parseErr) {
        // Fallback simple listing
        resolve([{ path: archivePath, uncompressedSize: 0, compressedSize: 0, crc32: 0, isDirectory: false, isEncrypted: false }]);
      }
    });
  });
}

module.exports = {
  compress,
  extract,
  inspect,
  crc32,
  version
};
