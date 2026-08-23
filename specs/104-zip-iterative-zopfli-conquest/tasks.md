# Tasks: Feature 104 (ZIP Iterative Zopfli Conquest)

## Phase 1: Native C Iterative Engine & Fixed-Point DP (US1)
- [x] T001 [US1] Update `ttzip_zopfli_engine.h` in `Sources/CTTZipBridge/include/ttzip_zopfli_engine.h` with enhanced `TTZipZopfliOptions` and thread context declarations.
- [x] T002 [US1] Implement Q8.8 fixed-point `CLZ` log2 table, dynamic Huffman symbol re-weighting, and 32KB history warmup in `Sources/CTTZipBridge/ttzip_zopfli_engine.c`.
- [x] T003 [US1] Implement 64-bit decision vector hashing and 0.005% marginal delta early-exit in `Sources/CTTZipBridge/ttzip_zopfli_engine.c`.

## Phase 2: Swift 18-Core Multi-Block Parallel Integration (US2)
- [x] T004 [US2] Update `ZipExtremeBlockWriter.swift` in `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift` to pass 32KB history pointer into C engine for 2MB tile multi-block parallel compression.
- [x] T005 [US2] Verify PKWARE Method 8 RFC 1951 stream compliance and run `swift test --filter ZipExtremeBlockWriterTests`.

## Phase 3: Benchmark & Pareto Frontier Verification (US3)
- [x] T006 [US3] Run `swift test --filter ZipMultiCoreParetoFrontierPkTests` to generate the new Pareto frontier chart and verify strict dominance of Tier 6 over `pigz -11` and Tier 7 over `advzip -4`.
- [x] T007 [US3] Verify zero regression across all 13 hard performance gates via `swift test --filter XCTestPerformanceMeasureTests`.
