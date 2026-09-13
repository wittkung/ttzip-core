// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Abstract asynchronous device storage driver contracts.

use crate::error::DeviceError;
use crate::models::AndroidVfsNode;
use bytes::Bytes;
use std::future::Future;
use std::pin::Pin;

/// Dynamic boxed future alias for dyn-compatible asynchronous driver methods.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Unified asynchronous driver interface for interacting with remote Android storage volumes.
///
/// Implementations (such as MTP or ADB SYNC) must be thread-safe (`Send + Sync`) and object-safe
/// (`dyn DeviceStorageDriver`) suitable for dynamic dispatch and multi-threaded executor scheduling.
pub trait DeviceStorageDriver: Send + Sync {
    /// Lists virtual filesystem entries within the directory at the specified remote path.
    fn list_directory<'a>(
        &'a self,
        path: &'a str,
    ) -> BoxFuture<'a, Result<Vec<AndroidVfsNode>, DeviceError>>;

    /// Reads a ranged byte slice from a remote object without downloading the entire payload.
    fn get_partial_object<'a>(
        &'a self,
        path: &'a str,
        offset: u64,
        length: u32,
    ) -> BoxFuture<'a, Result<Bytes, DeviceError>>;

    /// Transmits data payload directly to the specified destination path on the remote device.
    fn send_object<'a>(
        &'a self,
        destination_path: &'a str,
        data: Bytes,
    ) -> BoxFuture<'a, Result<(), DeviceError>>;

    /// Deletes a file or directory at the specified remote path.
    fn delete_object<'a>(
        &'a self,
        path: &'a str,
    ) -> BoxFuture<'a, Result<(), DeviceError>>;

    /// Gracefully terminates the session, releases USB interface claims, and flushes pending writes.
    fn disconnect<'a>(&'a self) -> BoxFuture<'a, Result<(), DeviceError>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockDriver;

    impl DeviceStorageDriver for MockDriver {
        fn list_directory<'a>(
            &'a self,
            _path: &'a str,
        ) -> BoxFuture<'a, Result<Vec<AndroidVfsNode>, DeviceError>> {
            Box::pin(async move { Ok(vec![]) })
        }

        fn get_partial_object<'a>(
            &'a self,
            _path: &'a str,
            _offset: u64,
            _length: u32,
        ) -> BoxFuture<'a, Result<Bytes, DeviceError>> {
            Box::pin(async move { Ok(Bytes::from_static(b"PK\x05\x06")) })
        }

        fn send_object<'a>(
            &'a self,
            _destination_path: &'a str,
            _data: Bytes,
        ) -> BoxFuture<'a, Result<(), DeviceError>> {
            Box::pin(async move { Ok(()) })
        }

        fn delete_object<'a>(
            &'a self,
            _path: &'a str,
        ) -> BoxFuture<'a, Result<(), DeviceError>> {
            Box::pin(async move { Ok(()) })
        }

        fn disconnect<'a>(&'a self) -> BoxFuture<'a, Result<(), DeviceError>> {
            Box::pin(async move { Ok(()) })
        }
    }

    #[tokio::test]
    async fn test_mock_driver_contracts() {
        let driver = MockDriver;
        let list = driver.list_directory("/").await.expect("Failed to list");
        assert!(list.is_empty());

        let bytes = driver
            .get_partial_object("/archive.zip", 0, 4)
            .await
            .expect("Failed to read");
        assert_eq!(&bytes[..], b"PK\x05\x06");

        driver
            .send_object("/Download/test.txt", Bytes::from_static(b"hello"))
            .await
            .expect("Failed to send");
        driver
            .delete_object("/Download/test.txt")
            .await
            .expect("Failed to delete");
        driver.disconnect().await.expect("Failed to disconnect");
    }
}
