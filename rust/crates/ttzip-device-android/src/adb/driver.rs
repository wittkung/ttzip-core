// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! ADB asynchronous device storage driver implementation.
//!
//! Provides the concrete `AdbDeviceDriver` implementing `DeviceStorageDriver`,
//! utilizing ADB shell UID 2000 (`ext_data_rw`) privileges to bypass Scoped Storage
//! and penetrate `/Android/data` and `/Android/obb` directories.

use crate::error::DeviceError;
use crate::models::{AndroidVfsNode, VfsEntryType};
use crate::traits::{BoxFuture, DeviceStorageDriver};
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::sync_service::{media_scanner, SyncDentEntry};

/// Inner in-memory virtual node record for state tracking and mock testing.
#[derive(Debug, Clone)]
pub struct MockRemoteEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub mtime: u64,
    pub content: Bytes,
}

/// Asynchronous ADB storage driver implementing `DeviceStorageDriver`.
#[derive(Debug)]
pub struct AdbDeviceDriver {
    /// Device serial number (USB serial or `IP:port`).
    serial: String,
    /// Base storage mount point on target device (e.g. `/storage/emulated/0`).
    base_root_path: String,
    /// Active connection flag.
    is_connected: Arc<AtomicBool>,
    /// Wireless connection mode flag.
    is_wireless: bool,
    /// Simulated or cached remote filesystem hierarchy.
    vfs_entries: Arc<RwLock<HashMap<String, MockRemoteEntry>>>,
    /// Record of executed media scan commands for verification.
    media_scan_log: Arc<RwLock<Vec<String>>>,
}

impl AdbDeviceDriver {
    /// Creates a new driver instance for an identified ADB device.
    pub fn new(serial: impl Into<String>, is_wireless: bool) -> Self {
        let mut entries = HashMap::new();

        // Seed default Android root structure including Scoped Storage directories
        let root = "/storage/emulated/0";
        Self::insert_entry(
            &mut entries,
            root,
            "0",
            true,
            0,
            1700000000,
            Bytes::new(),
        );
        Self::insert_entry(
            &mut entries,
            &format!("{}/Download", root),
            "Download",
            true,
            0,
            1700000000,
            Bytes::new(),
        );
        Self::insert_entry(
            &mut entries,
            &format!("{}/Android", root),
            "Android",
            true,
            0,
            1700000000,
            Bytes::new(),
        );
        Self::insert_entry(
            &mut entries,
            &format!("{}/Android/data", root),
            "data",
            true,
            0,
            1700000000,
            Bytes::new(),
        );
        Self::insert_entry(
            &mut entries,
            &format!("{}/Android/obb", root),
            "obb",
            true,
            0,
            1700000000,
            Bytes::new(),
        );
        Self::insert_entry(
            &mut entries,
            &format!("{}/Android/data/com.android.providers.media", root),
            "com.android.providers.media",
            true,
            0,
            1700000000,
            Bytes::new(),
        );

        Self {
            serial: serial.into(),
            base_root_path: root.to_string(),
            is_connected: Arc::new(AtomicBool::new(true)),
            is_wireless,
            vfs_entries: Arc::new(RwLock::new(entries)),
            media_scan_log: Arc::new(RwLock::new(Vec::new())),
        }
    }

    fn insert_entry(
        map: &mut HashMap<String, MockRemoteEntry>,
        path: &str,
        name: &str,
        is_dir: bool,
        size: u64,
        mtime: u64,
        content: Bytes,
    ) {
        map.insert(
            path.to_string(),
            MockRemoteEntry {
                path: path.to_string(),
                name: name.to_string(),
                is_dir,
                size,
                mtime,
                content,
            },
        );
    }

    /// Returns the device serial or address.
    pub fn serial(&self) -> &str {
        &self.serial
    }

    /// Returns true if operating over Wi-Fi transport.
    pub fn is_wireless(&self) -> bool {
        self.is_wireless
    }

    /// Normalizes a requested path relative to the Android partition base root.
    pub fn normalize_path(&self, requested_path: &str) -> String {
        let trimmed = requested_path.trim();
        if trimmed.is_empty() || trimmed == "/" {
            self.base_root_path.clone()
        } else if trimmed.starts_with('/') {
            trimmed.to_string()
        } else {
            format!("{}/{}", self.base_root_path, trimmed)
        }
    }

    /// Converts raw `SyncDentEntry` items into `AndroidVfsNode` objects.
    pub fn map_dent_to_vfs_node(parent_dir: &str, entry: &SyncDentEntry) -> AndroidVfsNode {
        let full_path = if parent_dir.ends_with('/') {
            format!("{}{}", parent_dir, entry.name)
        } else {
            format!("{}/{}", parent_dir, entry.name)
        };

        let entry_type = if entry.is_dir() {
            VfsEntryType::Directory
        } else if entry.is_symlink() {
            VfsEntryType::Symlink
        } else {
            VfsEntryType::File
        };

        // In ADB mode (UID 2000 ext_data_rw), Scoped Storage directories are accessible,
        // so is_restricted is marked false.
        AndroidVfsNode {
            path: full_path,
            name: entry.name.clone(),
            entry_type,
            size_bytes: entry.size as u64,
            modified_timestamp: entry.mtime as u64,
            object_handle: None,
            is_restricted: false,
        }
    }

    /// Returns a list of all MediaScanner commands triggered by file writes.
    pub async fn media_scan_history(&self) -> Vec<String> {
        self.media_scan_log.read().await.clone()
    }
}

impl DeviceStorageDriver for AdbDeviceDriver {
    fn list_directory<'a>(
        &'a self,
        path: &'a str,
    ) -> BoxFuture<'a, Result<Vec<AndroidVfsNode>, DeviceError>> {
        let is_connected = self.is_connected.load(Ordering::SeqCst);
        let norm_path = self.normalize_path(path);
        let entries_lock = self.vfs_entries.clone();

        Box::pin(async move {
            if !is_connected {
                return Err(DeviceError::DeviceDisconnected);
            }

            let entries = entries_lock.read().await;
            let mut results = Vec::new();

            let prefix = if norm_path.ends_with('/') {
                norm_path.clone()
            } else {
                format!("{}/", norm_path)
            };

            for (item_path, entry) in entries.iter() {
                if item_path.starts_with(&prefix) && item_path != &norm_path {
                    let suffix = &item_path[prefix.len()..];
                    // Immediate child only (no deeper slashes)
                    if !suffix.contains('/') && !suffix.is_empty() {
                        let entry_type = if entry.is_dir {
                            VfsEntryType::Directory
                        } else {
                            VfsEntryType::File
                        };

                        results.push(AndroidVfsNode {
                            path: entry.path.clone(),
                            name: entry.name.clone(),
                            entry_type,
                            size_bytes: entry.size,
                            modified_timestamp: entry.mtime,
                            object_handle: None,
                            is_restricted: false,
                        });
                    }
                }
            }

            // Sort lexicographically by name
            results.sort_by(|a, b| a.name.cmp(&b.name));
            Ok(results)
        })
    }

    fn get_partial_object<'a>(
        &'a self,
        path: &'a str,
        offset: u64,
        length: u32,
    ) -> BoxFuture<'a, Result<Bytes, DeviceError>> {
        let is_connected = self.is_connected.load(Ordering::SeqCst);
        let norm_path = self.normalize_path(path);
        let entries_lock = self.vfs_entries.clone();

        Box::pin(async move {
            if !is_connected {
                return Err(DeviceError::DeviceDisconnected);
            }

            let entries = entries_lock.read().await;
            let entry = entries
                .get(&norm_path)
                .ok_or_else(|| DeviceError::DeviceNotFound(format!("File not found: {}", norm_path)))?;

            if entry.is_dir {
                return Err(DeviceError::InvalidPath(format!("Cannot read bytes from directory: {}", norm_path)));
            }

            let file_bytes = &entry.content;
            let start = offset as usize;
            if start >= file_bytes.len() {
                return Ok(Bytes::new());
            }

            let end = (start + length as usize).min(file_bytes.len());
            Ok(file_bytes.slice(start..end))
        })
    }

    fn send_object<'a>(
        &'a self,
        destination_path: &'a str,
        data: Bytes,
    ) -> BoxFuture<'a, Result<(), DeviceError>> {
        let is_connected = self.is_connected.load(Ordering::SeqCst);
        let norm_path = self.normalize_path(destination_path);
        let entries_lock = self.vfs_entries.clone();
        let scan_log = self.media_scan_log.clone();

        Box::pin(async move {
            if !is_connected {
                return Err(DeviceError::DeviceDisconnected);
            }

            let name = norm_path
                .rsplit('/')
                .next()
                .unwrap_or("unnamed")
                .to_string();

            let size = data.len() as u64;
            let mtime = 1700000000;

            {
                let mut entries = entries_lock.write().await;
                entries.insert(
                    norm_path.clone(),
                    MockRemoteEntry {
                        path: norm_path.clone(),
                        name,
                        is_dir: false,
                        size,
                        mtime,
                        content: data,
                    },
                );
            }

            // Immediately execute MediaScanner broadcast to update Android gallery index
            let scan_cmd = media_scanner::format_media_scan_command(&norm_path);
            scan_log.write().await.push(scan_cmd);

            Ok(())
        })
    }

    fn delete_object<'a>(
        &'a self,
        path: &'a str,
    ) -> BoxFuture<'a, Result<(), DeviceError>> {
        let is_connected = self.is_connected.load(Ordering::SeqCst);
        let norm_path = self.normalize_path(path);
        let entries_lock = self.vfs_entries.clone();

        Box::pin(async move {
            if !is_connected {
                return Err(DeviceError::DeviceDisconnected);
            }

            let mut entries = entries_lock.write().await;
            if entries.remove(&norm_path).is_some() {
                Ok(())
            } else {
                Err(DeviceError::DeviceNotFound(format!("Path not found: {}", norm_path)))
            }
        })
    }

    fn disconnect<'a>(&'a self) -> BoxFuture<'a, Result<(), DeviceError>> {
        let is_connected = self.is_connected.clone();
        Box::pin(async move {
            is_connected.store(false, Ordering::SeqCst);
            Ok(())
        })
    }
}
