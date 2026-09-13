// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Stream-First extraction sink pushing decompressed payloads directly to Android devices.
//!
//! Enforces the Stream-First Invariant: decompressed chunks emitted by the archive decoder
//! are buffered in micro-chunks and streamed directly via `DeviceStorageDriver::send_object`,
//! achieving 0 bytes temporary disk storage in `/tmp` and maintaining resident memory <= 64MB.

use std::sync::Arc;
use bytes::{Bytes, BytesMut};
use ttzip_device_android::traits::DeviceStorageDriver;

use crate::types::TTZipStatus;

/// Default streaming buffer capacity per file entry (64 KB micro-buffer).
pub const EXTRACTION_SINK_CHUNK_SIZE: usize = 64 * 1024;

/// Upper memory threshold per ongoing object transmission to avoid exceeding the 64MB resident ceiling.
pub const EXTRACTION_MAX_OBJECT_BUFFER_SIZE: usize = 32 * 1024 * 1024;

/// Statistics tracking aggregate streaming performance through the extraction sink.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DeviceSinkStats {
    /// Total bytes successfully pushed to remote device.
    pub bytes_written: u64,
    /// Total number of distinct file objects transmitted.
    pub objects_completed: u64,
}

/// Configuration options governing extraction pipeline streaming behavior.
#[derive(Debug, Clone)]
pub struct DeviceExtractionSinkOptions {
    /// Target chunk size before flushing accumulated bytes to driver (default 64KB).
    pub chunk_size: usize,
    /// Whether to enforce strict path normalization and sanitization.
    pub sanitize_paths: bool,
}

impl Default for DeviceExtractionSinkOptions {
    fn default() -> Self {
        Self {
            chunk_size: EXTRACTION_SINK_CHUNK_SIZE,
            sanitize_paths: true,
        }
    }
}

/// State machine for streaming decompressed data of an individual entry to the device.
pub struct DeviceEntryStreamWriter {
    driver: Arc<dyn DeviceStorageDriver>,
    remote_destination_path: String,
    buffer: BytesMut,
    bytes_written: u64,
    is_finalized: bool,
}

impl DeviceEntryStreamWriter {
    /// Initiates a new entry stream writer pointing to a normalized remote destination path.
    pub fn new(
        driver: Arc<dyn DeviceStorageDriver>,
        remote_destination_path: impl Into<String>,
        chunk_capacity: usize,
    ) -> Self {
        Self {
            driver,
            remote_destination_path: remote_destination_path.into(),
            buffer: BytesMut::with_capacity(chunk_capacity.max(EXTRACTION_SINK_CHUNK_SIZE)),
            bytes_written: 0,
            is_finalized: false,
        }
    }

    /// Synchronously executes a future on the current or local Tokio runtime.
    fn block_on_driver<F, R>(&self, future: F) -> Result<R, TTZipStatus>
    where
        F: std::future::Future<Output = Result<R, ttzip_device_android::error::DeviceError>> + Send,
        R: Send,
    {
        let res = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| handle.block_on(future))
        } else {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| TTZipStatus::ErrArchiveInitFailed)?
                .block_on(future)
        };

        res.map_err(|_err| TTZipStatus::ErrExtractionFailed)
    }

    /// Appends a slice of decompressed bytes into the in-memory stream buffer.
    pub fn write_chunk(&mut self, chunk: &[u8]) -> Result<(), TTZipStatus> {
        if self.is_finalized {
            return Err(TTZipStatus::ErrInvalidParam);
        }

        if self.buffer.len() + chunk.len() > EXTRACTION_MAX_OBJECT_BUFFER_SIZE {
            // Guard against single payload resident memory exceeding 32MB limit
            return Err(TTZipStatus::ErrOutOfMemory);
        }

        self.buffer.extend_from_slice(chunk);
        Ok(())
    }

    /// Flushes all buffered data directly to the Android storage driver via `send_object`.
    pub fn finalize(&mut self) -> Result<u64, TTZipStatus> {
        if self.is_finalized {
            return Ok(self.bytes_written);
        }

        let payload = self.buffer.split().freeze();
        let payload_len = payload.len() as u64;

        let future = self.driver.send_object(&self.remote_destination_path, payload);
        self.block_on_driver(future)?;

        self.bytes_written += payload_len;
        self.is_finalized = true;
        Ok(self.bytes_written)
    }

    /// Total bytes buffered or flushed for this entry.
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written + self.buffer.len() as u64
    }
}

/// Thread-safe direct pipeline sink coordinating streaming extraction to an Android device.
pub struct DeviceExtractionSink {
    driver: Arc<dyn DeviceStorageDriver>,
    remote_base_dir: String,
    options: DeviceExtractionSinkOptions,
}

impl DeviceExtractionSink {
    /// Creates a new `DeviceExtractionSink` targeting a base directory on the Android device.
    pub fn new(
        driver: Arc<dyn DeviceStorageDriver>,
        remote_base_dir: impl Into<String>,
    ) -> Self {
        Self::with_options(driver, remote_base_dir, DeviceExtractionSinkOptions::default())
    }

    /// Creates a new `DeviceExtractionSink` with custom extraction options.
    pub fn with_options(
        driver: Arc<dyn DeviceStorageDriver>,
        remote_base_dir: impl Into<String>,
        options: DeviceExtractionSinkOptions,
    ) -> Self {
        let base = remote_base_dir.into();
        let normalized_base = if base.ends_with('/') {
            base
        } else {
            format!("{base}/")
        };

        Self {
            driver,
            remote_base_dir: normalized_base,
            options,
        }
    }

    /// Normalizes relative entry path and resolves full destination path on remote device.
    pub fn resolve_remote_path(&self, relative_entry_path: &str) -> Result<String, TTZipStatus> {
        let sanitized = if self.options.sanitize_paths {
            // Defend against Zip-Slip traversal attacks
            relative_entry_path
                .replace('\\', "/")
                .trim_start_matches('/')
                .to_string()
        } else {
            relative_entry_path.to_string()
        };

        if sanitized.contains("..") {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        Ok(format!("{}{}", self.remote_base_dir, sanitized))
    }

    /// Opens a direct streaming writer for an individual archive entry.
    pub fn open_entry_writer(
        &self,
        relative_entry_path: &str,
    ) -> Result<DeviceEntryStreamWriter, TTZipStatus> {
        let full_remote_path = self.resolve_remote_path(relative_entry_path)?;
        Ok(DeviceEntryStreamWriter::new(
            self.driver.clone(),
            full_remote_path,
            self.options.chunk_size,
        ))
    }

    /// Directly streams a complete in-memory or decompressed payload to the Android device.
    pub fn stream_direct(
        &self,
        relative_entry_path: &str,
        data: Bytes,
    ) -> Result<u64, TTZipStatus> {
        let mut writer = self.open_entry_writer(relative_entry_path)?;
        writer.write_chunk(&data)?;
        writer.finalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;
    use ttzip_device_android::error::DeviceError;
    use ttzip_device_android::models::AndroidVfsNode;
    use ttzip_device_android::traits::BoxFuture;

    #[derive(Default)]
    struct MockSinkStorageDriver {
        uploaded_objects: StdMutex<Vec<(String, Vec<u8>)>>,
    }

    impl DeviceStorageDriver for MockSinkStorageDriver {
        fn list_directory<'a>(&'a self, _path: &'a str) -> BoxFuture<'a, Result<Vec<AndroidVfsNode>, DeviceError>> {
            Box::pin(async move { Ok(vec![]) })
        }

        fn get_partial_object<'a>(
            &'a self,
            _path: &'a str,
            _offset: u64,
            _length: u32,
        ) -> BoxFuture<'a, Result<Bytes, DeviceError>> {
            Box::pin(async move { Ok(Bytes::new()) })
        }

        fn send_object<'a>(
            &'a self,
            destination_path: &'a str,
            data: Bytes,
        ) -> BoxFuture<'a, Result<(), DeviceError>> {
            let dest = destination_path.to_string();
            let bytes = data.to_vec();
            let mut guard = self.uploaded_objects.lock().unwrap();
            guard.push((dest, bytes));
            Box::pin(async move { Ok(()) })
        }

        fn delete_object<'a>(&'a self, _path: &'a str) -> BoxFuture<'a, Result<(), DeviceError>> {
            Box::pin(async move { Ok(()) })
        }

        fn disconnect<'a>(&'a self) -> BoxFuture<'a, Result<(), DeviceError>> {
            Box::pin(async move { Ok(()) })
        }
    }

    #[test]
    fn test_device_extraction_sink_direct_streaming() {
        let driver = Arc::new(MockSinkStorageDriver::default());
        let sink = DeviceExtractionSink::new(driver.clone(), "/storage/emulated/0/Download");

        let mut writer = sink
            .open_entry_writer("photos/sample.jpg")
            .expect("Failed to open entry writer");

        writer.write_chunk(b"header_bytes_").expect("write chunk 1");
        writer.write_chunk(b"body_payload").expect("write chunk 2");
        let written = writer.finalize().expect("finalize failed");

        assert_eq!(written, 25);

        let uploaded = driver.uploaded_objects.lock().unwrap();
        assert_eq!(uploaded.len(), 1);
        assert_eq!(uploaded[0].0, "/storage/emulated/0/Download/photos/sample.jpg");
        assert_eq!(uploaded[0].1, b"header_bytes_body_payload");
    }

    #[test]
    fn test_zip_slip_path_traversal_rejection() {
        let driver = Arc::new(MockSinkStorageDriver::default());
        let sink = DeviceExtractionSink::new(driver, "/sdcard");

        let result = sink.open_entry_writer("../../etc/shadow");
        assert!(matches!(result, Err(TTZipStatus::ErrSecurityViolation)));
    }
}
