// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! macOS 32-byte Finder Information structure (16-byte FInfo + 16-byte FXInfo).

/// macOS 32-byte Finder Information structure (16-byte `FInfo` + 16-byte `FXInfo`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FinderInfo(pub [u8; 32]);

impl FinderInfo {
    /// Creates a new zero-initialized FinderInfo structure.
    #[must_use]
    pub const fn new() -> Self {
        Self([0u8; 32])
    }

    /// Creates a FinderInfo instance from raw 32 bytes.
    #[must_use]
    pub const fn from_raw(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns a reference to the raw 32 bytes.
    #[must_use]
    pub const fn raw(&self) -> &[u8; 32] {
        &self.0
    }

    /// Gets the 4-byte OSType File Type (e.g. `b"TEXT"`, `b"APPL"`).
    #[must_use]
    pub fn file_type(&self) -> [u8; 4] {
        [self.0[0], self.0[1], self.0[2], self.0[3]]
    }

    /// Sets the 4-byte OSType File Type.
    pub fn set_file_type(&mut self, type_code: [u8; 4]) {
        self.0[0..4].copy_from_slice(&type_code);
    }

    /// Gets the 4-byte OSType Creator code.
    #[must_use]
    pub fn file_creator(&self) -> [u8; 4] {
        [self.0[4], self.0[5], self.0[6], self.0[7]]
    }

    /// Sets the 4-byte OSType Creator code.
    pub fn set_file_creator(&mut self, creator_code: [u8; 4]) {
        self.0[4..8].copy_from_slice(&creator_code);
    }

    /// Gets the 16-bit Big-Endian Finder flags.
    #[must_use]
    pub fn finder_flags(&self) -> u16 {
        u16::from_be_bytes([self.0[8], self.0[9]])
    }

    /// Sets the 16-bit Big-Endian Finder flags.
    pub fn set_finder_flags(&mut self, flags: u16) {
        let b = flags.to_be_bytes();
        self.0[8] = b[0];
        self.0[9] = b[1];
    }

    /// Returns true if the file is marked invisible (flag `0x4000`).
    #[must_use]
    pub fn is_invisible(&self) -> bool {
        (self.finder_flags() & 0x4000) != 0
    }

    /// Sets the invisible flag state.
    pub fn set_invisible(&mut self, invisible: bool) {
        let mut flags = self.finder_flags();
        if invisible {
            flags |= 0x4000;
        } else {
            flags &= !0x4000;
        }
        self.set_finder_flags(flags);
    }

    /// Returns true if the file has a custom icon (flag `0x0400`).
    #[must_use]
    pub fn has_custom_icon(&self) -> bool {
        (self.finder_flags() & 0x0400) != 0
    }

    /// Sets the custom icon flag state.
    pub fn set_custom_icon(&mut self, custom: bool) {
        let mut flags = self.finder_flags();
        if custom {
            flags |= 0x0400;
        } else {
            flags &= !0x0400;
        }
        self.set_finder_flags(flags);
    }

    /// Returns true if the file has a bundle bit set (flag `0x2000`).
    #[must_use]
    pub fn has_bundle(&self) -> bool {
        (self.finder_flags() & 0x2000) != 0
    }

    /// Sets the bundle bit state.
    pub fn set_bundle(&mut self, bundle: bool) {
        let mut flags = self.finder_flags();
        if bundle {
            flags |= 0x2000;
        } else {
            flags &= !0x2000;
        }
        self.set_finder_flags(flags);
    }

    /// Gets icon location point `(v, h)`.
    #[must_use]
    pub fn location(&self) -> (i16, i16) {
        let v = i16::from_be_bytes([self.0[10], self.0[11]]);
        let h = i16::from_be_bytes([self.0[12], self.0[13]]);
        (v, h)
    }

    /// Sets icon location point `(v, h)`.
    pub fn set_location(&mut self, v: i16, h: i16) {
        let vb = v.to_be_bytes();
        let hb = h.to_be_bytes();
        self.0[10] = vb[0];
        self.0[11] = vb[1];
        self.0[12] = hb[0];
        self.0[13] = hb[1];
    }

    /// Gets the 16-bit Big-Endian Extended Finder flags.
    #[must_use]
    pub fn extended_flags(&self) -> u16 {
        u16::from_be_bytes([self.0[24], self.0[25]])
    }

    /// Sets the 16-bit Big-Endian Extended Finder flags.
    pub fn set_extended_flags(&mut self, flags: u16) {
        let b = flags.to_be_bytes();
        self.0[24] = b[0];
        self.0[25] = b[1];
    }
}
