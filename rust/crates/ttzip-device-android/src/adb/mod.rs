// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Android Debug Bridge (ADB) protocol client and SYNC stream pipeline subsystem.
//!
//! Provides binary packet codecs, streaming filesystem directory traversal,
//! UID 2000 Scoped Storage penetration, and Wi-Fi zero-configuration SPAKE2 pairing.

pub mod driver;
pub mod protocol;
pub mod sync_service;
pub mod wireless_pairing;

// Re-export primary types for ergonomic usage
pub use driver::AdbDeviceDriver;
pub use protocol::{AdbCommand, AdbMessage, AdbPacket, ADB_MESSAGE_HEADER_SIZE, A_MAX_PAYLOAD};
pub use sync_service::{SyncDentEntry, SyncStatResponse, SYNC_CHUNK_SIZE};
pub use wireless_pairing::{
    PairingPacketHeader, PairingPacketType, PairingPeerInfo, Spake2ClientSession, Spake2HandshakeState,
    Spake2Pin,
};
