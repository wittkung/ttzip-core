// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import TTLogKit

extension TTLogCategory {
    public static let archive = TTLogCategory(rawValue: "archive")
    public static let preview = TTLogCategory(rawValue: "preview")
    public static let device  = TTLogCategory(rawValue: "device")
    public static let kernel  = TTLogCategory(rawValue: "kernel")
    public static let hang    = TTLogCategory(rawValue: "hang")
}
