// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.

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

/**
 * Creates an archive from input file paths.
 */
export function compress(inputs: string[], destination: string, options?: CompressOptions): Promise<void>;

/**
 * Extracts an archive to a destination directory.
 */
export function extract(archivePath: string, destination: string, options?: ExtractOptions): Promise<void>;

/**
 * Inspects an archive and returns structured file entry metadata.
 */
export function inspect(archivePath: string, password?: string): Promise<EntryMetadata[]>;

/**
 * Compresses an in-memory buffer using the specified algorithm.
 */
export function compressBuffer(data: Buffer, format?: string, level?: number): Buffer;

/**
 * Decompresses an in-memory buffer.
 */
export function decompressBuffer(data: Buffer, format?: string): Buffer;

/**
 * Computes hardware-accelerated CRC-32 checksum.
 */
export function crc32(data: Buffer): number;

/**
 * Returns TTZip engine version.
 */
export function version(): string;
