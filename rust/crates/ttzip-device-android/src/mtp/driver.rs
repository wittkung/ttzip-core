// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Asynchronous MTP device storage driver implementing `DeviceStorageDriver`.
//!
//! Maps flat MTP ObjectHandle hierarchies into standard POSIX paths,
//! enforces Android 11+ Scoped Storage access barriers (`/Android/data`),
//! and provides 64-bit chunked partial reads for instant archive inspection.

use crate::error::DeviceError;
use crate::models::{AndroidVfsNode, StoragePartition, VfsEntryType};
use crate::mtp::operations::{
    close_session, delete_object, get_object_handles, get_object_info, get_partial_object_64,
    get_storage_ids, get_storage_info, open_session, send_object, send_object_info, MtpInOut,
};
use crate::mtp::protocol::{
    decode_mtp_timestamp, encode_mtp_timestamp, MtpObjectInfo, FORMAT_UNDEFINED, MTP_PARENT_ROOT,
};
use crate::traits::{BoxFuture, DeviceStorageDriver};
use crate::transport::UsbTransport;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock};

/// Thread-safe asynchronous MTP storage driver managing USB Bulk communication sessions.
pub struct MtpDeviceDriver {
    io: Mutex<Box<dyn MtpInOut + Send>>,
    session_id: u32,
    transaction_id: AtomicU32,
    storage_id: u32,
    path_to_handle: RwLock<HashMap<String, u32>>,
    handle_to_path: RwLock<HashMap<u32, String>>,
}

impl MtpDeviceDriver {
    /// Creates a driver wrapping a physical `UsbTransport` and mounted storage partition.
    pub fn new(transport: UsbTransport, storage_id: u32) -> Self {
        Self::new_with_io(Box::new(transport), storage_id)
    }

    /// Creates a driver wrapping any `MtpInOut` transport implementation (useful for tests).
    pub fn new_with_io(io: Box<dyn MtpInOut + Send>, storage_id: u32) -> Self {
        let mut path_to_handle = HashMap::new();
        path_to_handle.insert("/".to_string(), MTP_PARENT_ROOT);
        path_to_handle.insert("".to_string(), MTP_PARENT_ROOT);

        Self {
            io: Mutex::new(io),
            session_id: 1,
            transaction_id: AtomicU32::new(100),
            storage_id,
            path_to_handle: RwLock::new(path_to_handle),
            handle_to_path: RwLock::new(HashMap::new()),
        }
    }

    /// Provides access to underlying mock I/O for integration tests.
    pub async fn io_for_test(&self) -> tokio::sync::MutexGuard<'_, Box<dyn MtpInOut + Send>> {
        self.io.lock().await
    }

    /// Connects to an attached USB transport, opens session, and identifies mounted partitions.
    pub async fn connect(mut transport: UsbTransport) -> Result<(Self, Vec<StoragePartition>), DeviceError> {
        let mut txn = 1;
        open_session(&mut transport, 1, &mut txn)?;

        let storage_ids = get_storage_ids(&mut transport, &mut txn)?;
        if storage_ids.is_empty() {
            return Err(DeviceError::ProtocolError(
                "Device reported zero mounted MTP storage partitions".to_string(),
            ));
        }

        let mut partitions = Vec::new();
        for &sid in &storage_ids {
            if let Ok(info) = get_storage_info(&mut transport, &mut txn, sid) {
                let name = if info.volume_identifier.is_empty() {
                    info.storage_description
                } else {
                    info.volume_identifier
                };
                partitions.push(StoragePartition {
                    partition_id: format!("0x{sid:08x}"),
                    display_name: if name.is_empty() { "Internal Storage".to_string() } else { name },
                    total_bytes: info.max_capacity,
                    available_bytes: info.free_space_bytes,
                    root_path: "/".to_string(),
                    is_removable: info.storage_type == 0x0002, // Removable RAM/Card
                });
            }
        }

        let primary_storage_id = storage_ids[0];
        let driver = Self::new(transport, primary_storage_id);
        Ok((driver, partitions))
    }

    /// Returns the currently active storage partition ID.
    pub fn storage_id(&self) -> u32 {
        self.storage_id
    }

    /// Checks whether a normalized POSIX path falls under Android 11+ Scoped Storage restrictions.
    pub fn is_scoped_storage_restricted(path: &str) -> bool {
        let norm = normalize_path(path);
        norm == "/Android/data"
            || norm.starts_with("/Android/data/")
            || norm == "/Android/obb"
            || norm.starts_with("/Android/obb/")
    }

    /// Resolves parent handle for a given remote directory path.
    async fn resolve_parent_handle(&self, path: &str) -> Result<u32, DeviceError> {
        let norm = normalize_path(path);
        if norm == "/" || norm.is_empty() {
            return Ok(MTP_PARENT_ROOT);
        }

        {
            let cache = self.path_to_handle.read().await;
            if let Some(&handle) = cache.get(&norm) {
                return Ok(handle);
            }
        }

        // Split path hierarchy and resolve incrementally from root
        let segments: Vec<&str> = norm.split('/').filter(|s| !s.is_empty()).collect();
        let mut current_parent = MTP_PARENT_ROOT;
        let mut current_path = String::new();

        for seg in segments {
            current_path.push('/');
            current_path.push_str(seg);

            let cached = {
                let cache = self.path_to_handle.read().await;
                cache.get(&current_path).copied()
            };

            if let Some(h) = cached {
                current_parent = h;
            } else {
                // Populate current directory level
                self.populate_directory_cache(current_parent, &current_path[..current_path.len() - seg.len() - 1]).await?;
                let cache = self.path_to_handle.read().await;
                if let Some(&h) = cache.get(&current_path) {
                    current_parent = h;
                } else {
                    return Err(DeviceError::InvalidPath(format!(
                        "Path segment '{seg}' not found in '{current_path}'"
                    )));
                }
            }
        }

        Ok(current_parent)
    }

    async fn populate_directory_cache(&self, parent_handle: u32, base_path: &str) -> Result<(), DeviceError> {
        let handles = {
            let mut io = self.io.lock().await;
            let mut txn = self.transaction_id.fetch_add(1, Ordering::SeqCst);
            get_object_handles(&mut **io, &mut txn, self.storage_id, parent_handle)?
        };

        for handle in handles {
            let info = {
                let mut io = self.io.lock().await;
                let mut txn = self.transaction_id.fetch_add(1, Ordering::SeqCst);
                get_object_info(&mut **io, &mut txn, handle)?
            };

            let item_path = if base_path == "/" || base_path.is_empty() {
                format!("/{}", info.filename)
            } else {
                format!("{base_path}/{}", info.filename)
            };

            let mut p_cache = self.path_to_handle.write().await;
            let mut h_cache = self.handle_to_path.write().await;
            p_cache.insert(item_path.clone(), handle);
            h_cache.insert(handle, item_path);
        }

        Ok(())
    }
}

impl DeviceStorageDriver for MtpDeviceDriver {
    fn list_directory<'a>(
        &'a self,
        path: &'a str,
    ) -> BoxFuture<'a, Result<Vec<AndroidVfsNode>, DeviceError>> {
        let norm_path = normalize_path(path);
        Box::pin(async move {
            let parent_handle = self.resolve_parent_handle(&norm_path).await?;

            let handles = {
                let mut io = self.io.lock().await;
                let mut txn = self.transaction_id.fetch_add(1, Ordering::SeqCst);
                get_object_handles(&mut **io, &mut txn, self.storage_id, parent_handle)?
            };

            let mut nodes = Vec::with_capacity(handles.len());
            for handle in handles {
                let info = {
                    let mut io = self.io.lock().await;
                    let mut txn = self.transaction_id.fetch_add(1, Ordering::SeqCst);
                    get_object_info(&mut **io, &mut txn, handle)?
                };

                let item_path = if norm_path == "/" {
                    format!("/{}", info.filename)
                } else {
                    format!("{norm_path}/{}", info.filename)
                };

                // Update bidirectional path caches
                {
                    let mut p_cache = self.path_to_handle.write().await;
                    let mut h_cache = self.handle_to_path.write().await;
                    p_cache.insert(item_path.clone(), handle);
                    h_cache.insert(handle, item_path.clone());
                }

                let is_dir = info.is_dir();
                let is_restricted = is_dir && Self::is_scoped_storage_restricted(&item_path);
                let entry_type = if is_restricted {
                    VfsEntryType::RestrictedDirectory
                } else if is_dir {
                    VfsEntryType::Directory
                } else {
                    VfsEntryType::File
                };

                let modified_timestamp = decode_mtp_timestamp(&info.date_modified);

                nodes.push(AndroidVfsNode {
                    path: item_path,
                    name: info.filename,
                    entry_type,
                    size_bytes: if is_dir { 0 } else { info.object_compressed_size as u64 },
                    modified_timestamp,
                    object_handle: Some(handle),
                    is_restricted,
                });
            }

            Ok(nodes)
        })
    }

    fn get_partial_object<'a>(
        &'a self,
        path: &'a str,
        offset: u64,
        length: u32,
    ) -> BoxFuture<'a, Result<Bytes, DeviceError>> {
        let norm_path = normalize_path(path);
        Box::pin(async move {
            if Self::is_scoped_storage_restricted(&norm_path) {
                return Err(DeviceError::RestrictedDirectoryAccess(norm_path));
            }

            let handle = self.resolve_parent_handle(&norm_path).await?;
            let mut io = self.io.lock().await;
            let mut txn = self.transaction_id.fetch_add(1, Ordering::SeqCst);

            get_partial_object_64(&mut **io, &mut txn, handle, offset, length)
        })
    }

    fn send_object<'a>(
        &'a self,
        destination_path: &'a str,
        data: Bytes,
    ) -> BoxFuture<'a, Result<(), DeviceError>> {
        let norm_path = normalize_path(destination_path);
        Box::pin(async move {
            if Self::is_scoped_storage_restricted(&norm_path) {
                return Err(DeviceError::RestrictedDirectoryAccess(norm_path));
            }

            let (parent_dir, filename) = split_path(&norm_path);
            let parent_handle = self.resolve_parent_handle(parent_dir).await?;

            let now_epoch = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let timestamp_str = encode_mtp_timestamp(now_epoch);

            let info = MtpObjectInfo {
                storage_id: self.storage_id,
                object_format: FORMAT_UNDEFINED,
                protection_status: 0,
                object_compressed_size: data.len() as u32,
                thumb_format: 0,
                thumb_compressed_size: 0,
                thumb_pix_width: 0,
                thumb_pix_height: 0,
                image_pix_width: 0,
                image_pix_height: 0,
                image_bit_depth: 0,
                parent_object: parent_handle,
                association_type: 0,
                association_desc: 0,
                sequence_number: 0,
                filename: filename.to_string(),
                date_created: timestamp_str.clone(),
                date_modified: timestamp_str,
                keywords: String::new(),
            };

            let new_handle = {
                let mut io = self.io.lock().await;
                let mut txn = self.transaction_id.fetch_add(1, Ordering::SeqCst);
                let handle = send_object_info(&mut **io, &mut txn, self.storage_id, parent_handle, &info)?;
                send_object(&mut **io, &mut txn, data)?;
                handle
            };

            // Register newly created object in cache
            {
                let mut p_cache = self.path_to_handle.write().await;
                let mut h_cache = self.handle_to_path.write().await;
                p_cache.insert(norm_path.clone(), new_handle);
                h_cache.insert(new_handle, norm_path);
            }

            Ok(())
        })
    }

    fn delete_object<'a>(
        &'a self,
        path: &'a str,
    ) -> BoxFuture<'a, Result<(), DeviceError>> {
        let norm_path = normalize_path(path);
        Box::pin(async move {
            if Self::is_scoped_storage_restricted(&norm_path) {
                return Err(DeviceError::RestrictedDirectoryAccess(norm_path));
            }

            let handle = self.resolve_parent_handle(&norm_path).await?;
            {
                let mut io = self.io.lock().await;
                let mut txn = self.transaction_id.fetch_add(1, Ordering::SeqCst);
                delete_object(&mut **io, &mut txn, handle)?;
            }

            // Remove from cache
            {
                let mut p_cache = self.path_to_handle.write().await;
                let mut h_cache = self.handle_to_path.write().await;
                p_cache.remove(&norm_path);
                h_cache.remove(&handle);
            }

            Ok(())
        })
    }

    fn disconnect<'a>(&'a self) -> BoxFuture<'a, Result<(), DeviceError>> {
        Box::pin(async move {
            let mut io = self.io.lock().await;
            let mut txn = self.transaction_id.fetch_add(1, Ordering::SeqCst);
            close_session(&mut **io, self.session_id, &mut txn)
        })
    }
}

fn normalize_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return "/".to_string();
    }
    let mut s = String::with_capacity(trimmed.len() + 1);
    if !trimmed.starts_with('/') {
        s.push('/');
    }
    s.push_str(trimmed.trim_end_matches('/'));
    s
}

fn split_path(path: &str) -> (&str, &str) {
    let norm = path.trim_end_matches('/');
    if let Some(pos) = norm.rfind('/') {
        let parent = if pos == 0 { "/" } else { &norm[..pos] };
        let name = &norm[pos + 1..];
        (parent, name)
    } else {
        ("/", norm)
    }
}
