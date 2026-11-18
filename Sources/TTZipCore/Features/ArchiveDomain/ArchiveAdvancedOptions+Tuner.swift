// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import TTZipCoreBindings
import Foundation
import TTZipOperationContract

extension ArchiveAdvancedOptions {
    public static var defaultOptions: ArchiveAdvancedOptions {
        let cores = AppleSiliconTuner.shared.topology.totalCores
        return ArchiveAdvancedOptions(
            cpuThreads: cores,
            recoveryRecordRedundancyPercent: 0,
            zipOptions: ZipFormatOptions(),
            sevenZipOptions: SevenZipFormatOptions(),
            zstdOptions: ZstdFormatOptions(),
            tarOptions: TarFormatOptions(),
            appleArchiveOptions: AppleArchiveFormatOptions(),
            diskImageOptions: DiskImageFormatOptions(),
            wimOptions: WimFormatOptions()
        )
    }
}
