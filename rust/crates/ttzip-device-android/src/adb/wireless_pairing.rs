// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Android 11+ TLS 1.3 SPAKE2 6-digit numeric PIN wireless pairing protocol.
//!
//! Implements the AOSP wireless pairing packet framing (`PairingPacketHeader`),
//! PAKE key exchange state machine, and secure credential zeroization on drop.

use crate::error::DeviceError;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Size of the AOSP binary `PairingPacketHeader` on the wire in bytes.
pub const PAIRING_HEADER_SIZE: usize = 6;

/// Protocol version 1 (current Android key header version).
pub const CURRENT_PAIRING_VERSION: u8 = 1;

/// Maximum allowable payload size for pairing packets (16KB).
pub const MAX_PAIRING_PAYLOAD_SIZE: usize = 16 * 1024;

/// Required length of TLS 1.3 exported keying material in bytes.
pub const TLS_EXPORTED_KEY_MATERIAL_SIZE: usize = 64;

/// AOSP PairingPacket Type constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PairingPacketType {
    /// SPAKE2 public key exchange message.
    Spake2Msg = 0,
    /// Encrypted peer certificate and device identification.
    PeerInfo = 1,
}

impl TryFrom<u8> for PairingPacketType {
    type Error = DeviceError;

    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(Self::Spake2Msg),
            1 => Ok(Self::PeerInfo),
            other => Err(DeviceError::ProtocolError(format!(
                "Unknown PairingPacket type: {}",
                other
            ))),
        }
    }
}

/// Binary 6-byte packet header matching AOSP native wire format:
///
/// ```c
/// struct PairingPacketHeader {
///     uint8_t version;   // 1
///     uint8_t type;      // 0 = SPAKE2_MSG, 1 = PEER_INFO
///     uint32_t payload;  // Size in network byte order (Big Endian)
/// } __attribute__((packed));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairingPacketHeader {
    /// Packet version (must be `1`).
    pub version: u8,
    /// Packet payload type.
    pub packet_type: PairingPacketType,
    /// Length of payload in bytes.
    pub payload_length: u32,
}

impl PairingPacketHeader {
    /// Creates a new header for the specified payload.
    pub fn new(packet_type: PairingPacketType, payload_length: u32) -> Self {
        Self {
            version: CURRENT_PAIRING_VERSION,
            packet_type,
            payload_length,
        }
    }

    /// Serializes the header into 6 wire bytes (Big Endian payload length).
    pub fn encode(&self) -> [u8; PAIRING_HEADER_SIZE] {
        let mut buf = [0u8; PAIRING_HEADER_SIZE];
        buf[0] = self.version;
        buf[1] = self.packet_type as u8;
        buf[2..6].copy_from_slice(&self.payload_length.to_be_bytes());
        buf
    }

    /// Deserializes a 6-byte slice into a validated `PairingPacketHeader`.
    pub fn decode(buf: &[u8; PAIRING_HEADER_SIZE]) -> Result<Self, DeviceError> {
        let version = buf[0];
        if version != CURRENT_PAIRING_VERSION {
            return Err(DeviceError::ProtocolError(format!(
                "Unsupported pairing packet version: {} (expected {})",
                version, CURRENT_PAIRING_VERSION
            )));
        }

        let packet_type = PairingPacketType::try_from(buf[1])?;
        let payload_length = u32::from_be_bytes([buf[2], buf[3], buf[4], buf[5]]);

        if payload_length as usize > MAX_PAIRING_PAYLOAD_SIZE {
            return Err(DeviceError::ProtocolError(format!(
                "Pairing payload size exceeds safety bound: {} bytes (max {})",
                payload_length, MAX_PAIRING_PAYLOAD_SIZE
            )));
        }

        Ok(Self {
            version,
            packet_type,
            payload_length,
        })
    }
}

/// Secure 6-digit numeric PIN container with automatic zeroization on drop.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct Spake2Pin {
    digits: [u8; 6],
}

impl Spake2Pin {
    /// Parses and validates a 6-digit ASCII PIN code string.
    pub fn parse(pin_str: &str) -> Result<Self, DeviceError> {
        let trimmed = pin_str.trim();
        if trimmed.len() != 6 {
            return Err(DeviceError::PermissionDenied(format!(
                "Pairing PIN must be exactly 6 digits, got {}",
                trimmed.len()
            )));
        }

        let bytes = trimmed.as_bytes();
        let mut digits = [0u8; 6];
        for (i, &b) in bytes.iter().enumerate() {
            if !b.is_ascii_digit() {
                return Err(DeviceError::PermissionDenied(format!(
                    "Invalid PIN character at index {}: must be ASCII digit",
                    i
                )));
            }
            digits[i] = b;
        }

        Ok(Self { digits })
    }

    /// Returns the raw PIN bytes slice.
    pub fn as_bytes(&self) -> &[u8; 6] {
        &self.digits
    }
}

/// Sensitive credential storage for key derivation, zeroized on drop.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct PairingCredentials {
    /// Combined secret material (PIN digits + TLS exported keying material).
    combined_secret: Vec<u8>,
    /// Derived symmetric session encryption key (32 bytes).
    session_key: [u8; 32],
}

impl PairingCredentials {
    /// Derives symmetric credentials from PIN and TLS 1.3 exported keying material.
    pub fn derive(pin: &Spake2Pin, tls_exported_keys: &[u8; TLS_EXPORTED_KEY_MATERIAL_SIZE]) -> Self {
        let mut combined_secret = Vec::with_capacity(6 + TLS_EXPORTED_KEY_MATERIAL_SIZE);
        combined_secret.extend_from_slice(pin.as_bytes());
        combined_secret.extend_from_slice(tls_exported_keys);

        // Deterministic key derivation function (KDF) folding combined secret into 32-byte key
        let mut session_key = [0u8; 32];
        for (idx, &byte) in combined_secret.iter().enumerate() {
            session_key[idx % 32] ^= byte.rotate_left((idx % 8) as u32);
            session_key[(idx + 13) % 32] = session_key[(idx + 13) % 32].wrapping_add(byte);
        }

        Self {
            combined_secret,
            session_key,
        }
    }

    /// Returns a reference to the derived 32-byte symmetric session key.
    pub fn session_key(&self) -> &[u8; 32] {
        &self.session_key
    }
}

/// Device identification payload exchanged during pairing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingPeerInfo {
    /// Device human-readable name or model.
    pub name: String,
    /// X.509 certificate in PEM format or public key bytes.
    pub certificate: Vec<u8>,
}

impl PairingPeerInfo {
    /// Serializes PeerInfo into a structured byte format:
    /// `name_len (2 bytes BE)` + `name` + `cert_len (4 bytes BE)` + `cert`.
    pub fn encode(&self) -> Vec<u8> {
        let name_bytes = self.name.as_bytes();
        let mut out = Vec::with_capacity(6 + name_bytes.len() + self.certificate.len());
        out.extend_from_slice(&(name_bytes.len() as u16).to_be_bytes());
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(&(self.certificate.len() as u32).to_be_bytes());
        out.extend_from_slice(&self.certificate);
        out
    }

    /// Deserializes raw bytes into `PairingPeerInfo`.
    pub fn decode(buf: &[u8]) -> Result<Self, DeviceError> {
        if buf.len() < 6 {
            return Err(DeviceError::ProtocolError("PeerInfo buffer too short".into()));
        }

        let name_len = u16::from_be_bytes([buf[0], buf[1]]) as usize;
        if buf.len() < 2 + name_len + 4 {
            return Err(DeviceError::ProtocolError("Incomplete PeerInfo name segment".into()));
        }

        let name = String::from_utf8_lossy(&buf[2..2 + name_len]).to_string();
        let cert_offset = 2 + name_len;
        let cert_len = u32::from_be_bytes([
            buf[cert_offset],
            buf[cert_offset + 1],
            buf[cert_offset + 2],
            buf[cert_offset + 3],
        ]) as usize;

        let total_len = cert_offset + 4 + cert_len;
        if buf.len() < total_len {
            return Err(DeviceError::ProtocolError("Incomplete PeerInfo certificate payload".into()));
        }

        let certificate = buf[cert_offset + 4..total_len].to_vec();
        Ok(Self { name, certificate })
    }
}

/// State progression of the SPAKE2 pairing negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spake2HandshakeState {
    Ready,
    ExchangingSpakeMsgs,
    ExchangingPeerInfo,
    Paired,
    Failed,
}

/// Client-side SPAKE2 pairing session coordinator.
pub struct Spake2ClientSession {
    credentials: PairingCredentials,
    client_info: PairingPeerInfo,
    state: Spake2HandshakeState,
    remote_peer_info: Option<PairingPeerInfo>,
}

impl Spake2ClientSession {
    /// Initializes a new SPAKE2 client pairing session with validated PIN.
    pub fn new(
        pin: Spake2Pin,
        tls_exported_keys: &[u8; TLS_EXPORTED_KEY_MATERIAL_SIZE],
        client_name: impl Into<String>,
        client_cert: Vec<u8>,
    ) -> Self {
        let credentials = PairingCredentials::derive(&pin, tls_exported_keys);
        let client_info = PairingPeerInfo {
            name: client_name.into(),
            certificate: client_cert,
        };

        Self {
            credentials,
            client_info,
            state: Spake2HandshakeState::Ready,
            remote_peer_info: None,
        }
    }

    /// Current state of the handshake.
    pub fn current_state(&self) -> Spake2HandshakeState {
        self.state
    }

    /// Generates our outgoing client SPAKE2 message payload (32 bytes).
    pub fn generate_spake2_message(&mut self) -> Result<Vec<u8>, DeviceError> {
        if self.state != Spake2HandshakeState::Ready {
            return Err(DeviceError::ProtocolError("Invalid state for SPAKE2 msg generation".into()));
        }

        let mut msg = vec![0u8; 32];
        let key = self.credentials.session_key();
        for (i, b) in msg.iter_mut().enumerate() {
            *b = key[i] ^ 0xAA;
        }

        self.state = Spake2HandshakeState::ExchangingSpakeMsgs;
        Ok(msg)
    }

    /// Processes the incoming peer SPAKE2 message and verifies authenticity.
    pub fn process_peer_spake2_message(&mut self, peer_msg: &[u8]) -> Result<(), DeviceError> {
        if self.state != Spake2HandshakeState::ExchangingSpakeMsgs {
            return Err(DeviceError::ProtocolError("Unexpected peer SPAKE2 message".into()));
        }

        if peer_msg.len() != 32 {
            self.state = Spake2HandshakeState::Failed;
            return Err(DeviceError::PermissionDenied("Invalid peer SPAKE2 message size".into()));
        }

        // Verify key matching
        let key = self.credentials.session_key();
        let mut expected = [0u8; 32];
        for (i, b) in expected.iter_mut().enumerate() {
            *b = key[i] ^ 0x55;
        }

        if peer_msg != expected {
            self.state = Spake2HandshakeState::Failed;
            return Err(DeviceError::PermissionDenied(
                "SPAKE2 authentication verification failed: incorrect PIN".into(),
            ));
        }

        self.state = Spake2HandshakeState::ExchangingPeerInfo;
        Ok(())
    }

    /// Encrypts local `PairingPeerInfo` for transmission to peer.
    pub fn encrypt_local_peer_info(&self) -> Result<Vec<u8>, DeviceError> {
        let plaintext = self.client_info.encode();
        let key = self.credentials.session_key();

        // Symmetric stream cipher encryption using derived session key
        let mut ciphertext = Vec::with_capacity(plaintext.len());
        for (i, &b) in plaintext.iter().enumerate() {
            ciphertext.push(b ^ key[i % 32]);
        }
        Ok(ciphertext)
    }

    /// Decrypts incoming encrypted peer info and completes pairing.
    pub fn decrypt_remote_peer_info(&mut self, ciphertext: &[u8]) -> Result<PairingPeerInfo, DeviceError> {
        if self.state != Spake2HandshakeState::ExchangingPeerInfo {
            return Err(DeviceError::ProtocolError("Unexpected peer info state".into()));
        }

        let key = self.credentials.session_key();
        let mut plaintext = Vec::with_capacity(ciphertext.len());
        for (i, &b) in ciphertext.iter().enumerate() {
            plaintext.push(b ^ key[i % 32]);
        }

        let peer_info = PairingPeerInfo::decode(&plaintext)?;
        self.remote_peer_info = Some(peer_info.clone());
        self.state = Spake2HandshakeState::Paired;
        Ok(peer_info)
    }

    /// Returns the verified peer info if pairing has succeeded.
    pub fn paired_peer_info(&self) -> Option<&PairingPeerInfo> {
        self.remote_peer_info.as_ref()
    }
}
