// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Container signatures, sniffer anchors, and magic detection rules.

pub mod anchors;
pub mod signatures;
pub mod sniffer;

pub use anchors::*;
pub use signatures::*;
pub use sniffer::*;
