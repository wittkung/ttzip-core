# Tasks: Entropy-Adaptive Intelligent Extreme Routing

- [ ] T001 [P] [US1] Add `ttzip_probe_entropy_and_compressibility` into `Sources/CTTZipBridge/include/CTTZipStreamCoder.h` and `Sources/CTTZipBridge/CTTZipStreamCoder.c`.
- [ ] T002 [US1] Update `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift` with entropy probe routing (Method 0 Store vs Method 8 Multi-Core Deflate).
- [ ] T003 [US1] Create `Tests/TTZipTests/EntropyAdaptiveExtremeRoutingTests.swift` to test low-entropy vs high-entropy routing and `/usr/bin/unzip -t` verification.
- [ ] T004 [US1] Run full local CI/CD gate and push.
