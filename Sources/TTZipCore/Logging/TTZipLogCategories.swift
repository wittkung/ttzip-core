// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import TTLogKit

extension TTLogCategory {
    public static let archive = TTLogCategory(rawValue: "archive")
    public static let preview = TTLogCategory(rawValue: "preview")
    public static let device  = TTLogCategory(rawValue: "device")
    public static let kernel  = TTLogCategory(rawValue: "kernel")
    public static let hang    = TTLogCategory(rawValue: "hang")
}
