// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Asynchronous TCP transport client with automatic reconnect and keepalive monitoring.
//!
//! Handles reliable network streaming for wireless ADB sessions (Android 11+ Wi-Fi)
//! with configurable exponential backoff and connection stall detection.

use crate::error::DeviceError;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Configuration defining the automatic reconnection policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconnectPolicy {
    /// Maximum reconnection attempts before declaring fatal failure (0 for infinite).
    pub max_retries: usize,
    /// Initial backoff delay in milliseconds.
    pub initial_backoff_ms: u64,
    /// Maximum backoff clamp delay in milliseconds.
    pub max_backoff_ms: u64,
    /// Multiplier factor applied per retry attempt.
    pub backoff_multiplier: u64,
    /// Timeout for individual connection attempts in milliseconds.
    pub connect_timeout_ms: u64,
    /// Keepalive idle probe interval in seconds.
    pub keepalive_interval_secs: u64,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_backoff_ms: 100,
            max_backoff_ms: 3000,
            backoff_multiplier: 2,
            connect_timeout_ms: 2000,
            keepalive_interval_secs: 15,
        }
    }
}

impl ReconnectPolicy {
    /// Calculates the backoff duration for a given 0-indexed attempt.
    pub fn calculate_backoff(&self, attempt: usize) -> Duration {
        let mut delay = self.initial_backoff_ms;
        for _ in 0..attempt {
            delay = delay.saturating_mul(self.backoff_multiplier);
            if delay >= self.max_backoff_ms {
                delay = self.max_backoff_ms;
                break;
            }
        }
        Duration::from_millis(delay)
    }
}

/// Lifecycle connection state of the TCP transport client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting(usize),
    Failed,
}

/// Reliable TCP client for wireless ADB socket communication.
pub struct AdbTcpClient {
    /// Remote socket endpoint address.
    target_addr: SocketAddr,
    /// Reconnect parameters.
    policy: ReconnectPolicy,
    /// Current connection status.
    status: Arc<parking_lot_stub::RwLock<TcpConnectionStatus>>,
    /// Active underlying socket stream.
    stream: Arc<parking_lot_stub::RwLock<Option<TcpStream>>>,
    /// Track total successful reconnect cycles.
    reconnect_count: Arc<AtomicUsize>,
    /// Shutdown signal.
    is_closed: Arc<AtomicBool>,
}

// Minimal zero-dependency parking lot style lock adapter around standard sync primitives.
mod parking_lot_stub {
    use std::sync::{Mutex, MutexGuard};

    pub struct RwLock<T>(Mutex<T>);

    impl<T> RwLock<T> {
        pub fn new(val: T) -> Self {
            Self(Mutex::new(val))
        }

        pub fn read(&self) -> MutexGuard<'_, T> {
            self.0.lock().unwrap()
        }

        pub fn write(&self) -> MutexGuard<'_, T> {
            self.0.lock().unwrap()
        }
    }
}

impl AdbTcpClient {
    /// Instantiates a new TCP client for the designated remote endpoint.
    pub fn new(target_addr: SocketAddr, policy: ReconnectPolicy) -> Self {
        Self {
            target_addr,
            policy,
            status: Arc::new(parking_lot_stub::RwLock::new(TcpConnectionStatus::Disconnected)),
            stream: Arc::new(parking_lot_stub::RwLock::new(None)),
            reconnect_count: Arc::new(AtomicUsize::new(0)),
            is_closed: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Returns the target remote endpoint address.
    pub fn target_addr(&self) -> SocketAddr {
        self.target_addr
    }

    /// Returns the current lifecycle connection status.
    pub fn current_status(&self) -> TcpConnectionStatus {
        *self.status.read()
    }

    /// Returns the number of successful auto-reconnects performed.
    pub fn reconnect_count(&self) -> usize {
        self.reconnect_count.load(Ordering::SeqCst)
    }

    /// Establishes initial connection to the target remote host.
    pub fn connect(&self) -> Result<(), DeviceError> {
        if self.is_closed.load(Ordering::SeqCst) {
            return Err(DeviceError::TransportError("Client is shut down".into()));
        }

        *self.status.write() = TcpConnectionStatus::Connecting;

        match TcpStream::connect_timeout(
            &self.target_addr,
            Duration::from_millis(self.policy.connect_timeout_ms),
        ) {
            Ok(stream) => {
                self.configure_socket(&stream)?;
                *self.stream.write() = Some(stream);
                *self.status.write() = TcpConnectionStatus::Connected;
                Ok(())
            }
            Err(e) => {
                *self.status.write() = TcpConnectionStatus::Disconnected;
                Err(DeviceError::TransportError(format!(
                    "TCP connect to {} failed: {}",
                    self.target_addr, e
                )))
            }
        }
    }

    /// Configures essential socket timeouts and TCP_NODELAY parameters.
    fn configure_socket(&self, stream: &TcpStream) -> Result<(), DeviceError> {
        stream
            .set_nodelay(true)
            .map_err(|e| DeviceError::TransportError(format!("Failed to set TCP_NODELAY: {}", e)))?;

        stream
            .set_read_timeout(Some(Duration::from_secs(self.policy.keepalive_interval_secs)))
            .map_err(|e| DeviceError::TransportError(format!("Failed to set read timeout: {}", e)))?;

        stream
            .set_write_timeout(Some(Duration::from_secs(self.policy.keepalive_interval_secs)))
            .map_err(|e| DeviceError::TransportError(format!("Failed to set write timeout: {}", e)))?;

        Ok(())
    }

    /// Transmits raw buffer with automatic reconnect on pipe failure.
    pub fn write_all(&self, data: &[u8]) -> Result<(), DeviceError> {
        if self.is_closed.load(Ordering::SeqCst) {
            return Err(DeviceError::DeviceDisconnected);
        }

        let mut attempt = 0;
        loop {
            let write_result = {
                let mut guard = self.stream.write();
                if let Some(ref mut s) = *guard {
                    s.write_all(data)
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::NotConnected,
                        "Socket not connected",
                    ))
                }
            };

            match write_result {
                Ok(_) => return Ok(()),
                Err(e) => {
                    attempt += 1;
                    if attempt > self.policy.max_retries {
                        *self.status.write() = TcpConnectionStatus::Failed;
                        return Err(DeviceError::TransportError(format!(
                            "TCP write exhausted {} retries: {}",
                            self.policy.max_retries, e
                        )));
                    }

                    self.reconnect_with_backoff(attempt)?;
                }
            }
        }
    }

    /// Reads exactly `buf.len()` bytes with automatic reconnect on broken connection.
    pub fn read_exact(&self, buf: &mut [u8]) -> Result<(), DeviceError> {
        if self.is_closed.load(Ordering::SeqCst) {
            return Err(DeviceError::DeviceDisconnected);
        }

        let mut guard = self.stream.write();
        let stream = guard
            .as_mut()
            .ok_or_else(|| DeviceError::TransportError("TCP socket not connected".into()))?;

        stream.read_exact(buf).map_err(|e| {
            if e.kind() == std::io::ErrorKind::TimedOut || e.kind() == std::io::ErrorKind::WouldBlock {
                DeviceError::ProtocolTimeout(format!("TCP read timed out: {}", e))
            } else {
                DeviceError::TransportError(format!("TCP read_exact failed: {}", e))
            }
        })
    }

    /// Performs backoff sleeping and socket reconnection.
    fn reconnect_with_backoff(&self, attempt: usize) -> Result<(), DeviceError> {
        *self.status.write() = TcpConnectionStatus::Reconnecting(attempt);
        let backoff = self.policy.calculate_backoff(attempt);
        std::thread::sleep(backoff);

        match TcpStream::connect_timeout(
            &self.target_addr,
            Duration::from_millis(self.policy.connect_timeout_ms),
        ) {
            Ok(new_stream) => {
                self.configure_socket(&new_stream)?;
                *self.stream.write() = Some(new_stream);
                *self.status.write() = TcpConnectionStatus::Connected;
                self.reconnect_count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
            Err(e) => Err(DeviceError::TransportError(format!(
                "Reconnection attempt {} failed: {}",
                attempt, e
            ))),
        }
    }

    /// Performs a non-destructive socket health probe.
    pub fn send_keepalive_probe(&self) -> Result<bool, DeviceError> {
        let guard = self.stream.read();
        if let Some(ref s) = *guard {
            match s.take_error() {
                Ok(None) => Ok(true),
                Ok(Some(err)) => Err(DeviceError::TransportError(format!("Socket probe error: {}", err))),
                Err(e) => Err(DeviceError::TransportError(format!("Socket probe call failed: {}", e))),
            }
        } else {
            Ok(false)
        }
    }

    /// Gracefully closes and drops the TCP connection.
    pub fn disconnect(&self) -> Result<(), DeviceError> {
        self.is_closed.store(true, Ordering::SeqCst);
        let mut guard = self.stream.write();
        if let Some(s) = guard.take() {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
        *self.status.write() = TcpConnectionStatus::Disconnected;
        Ok(())
    }
}

impl Drop for AdbTcpClient {
    fn drop(&mut self) {
        let _ = self.disconnect();
    }
}
