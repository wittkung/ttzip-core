# MM37: archive password confirmation

Stage 1 archive API is accepted on its isolated candidate based on `6702bb26a`: the formal archive target passed 328 tests, including 25 confirmation regressions, and strict offline clippy passed. Engine/FFI/UniFFI orchestration and TUI/CLI consumption remain separate dependent stages; adding this archive API alone does not fix their existing screening-only behavior. This local acceptance is not a complete release gate or GUI acceptance.

## Ownership and success contract

`ttzip-crypto` generates or screens candidates. A ZipCrypto header byte or WinZip AES PVV match is only a reason to perform content confirmation. It must never be displayed as a recovered password.

`ttzip-archive::recovery::PasswordConfirmation` parses an archive and selects one eligible encrypted unit once, then reuses its real extraction path for candidate attempts. The target must carry both encryption evidence and a verifiable integrity credential:

| Format | Encryption evidence | Confirmation path and credential |
| --- | --- | --- |
| ZipCrypto | Entry encryption flag, nonempty plaintext and complete encryption header | `ZipArchive::extract_entry_bytes`: actual decryption/decompression and entry CRC32, including a legitimate CRC value of zero |
| WinZip AES | Encrypted AES-256 entry (strength 3) with complete authentication payload | The same ZIP extraction path verifies HMAC-SHA1-80; AE-2 does not require a CRC, and authenticated empty content can carry evidence |
| 7z | AES coder in the selected entry's own folder, independently of the global encrypted-header flag | Existing `extract_single_entry_bounded`: actual decryption/decompression and the selected entry's `loc.crc`; a folder CRC cannot replace the entry credential during early termination |

The confirmation reader currently implements only AES-256. Valid AE-1/AE-2 strength 1 (AES-128) and 2 (AES-192) yield `Unsupported`, including authenticated empty entries; they never become `NotAccepted` or `NoCredential`. Automatic selection skips these unsupported units and may select a later supported credential-bearing unit. Invalid AES metadata (including strength 0) is `ArchiveDamaged`, independently of valid unsupported strengths. LFH/CDFH AES contradictions are checked during preparation with the same existing reader validator and return `ArchiveDamaged` before any candidate is tried. Ciphertext/PVV/HMAC or plaintext-integrity failures still yield `NotAccepted`; this does not relabel tampering as proven wrong passwords. No additional reader algorithm is introduced.

The proof covers only the selected encrypted unit. It does not establish a common password for every entry or prove the health of the entire archive. An empty ZipCrypto unit, or a 7z unit without an entry digest, supplies no acceptable content proof. 7z has no invented fixed-magic screen.

`Confirmed` is the sole positive result. `NotAccepted` deliberately does not distinguish an incorrect candidate from modified ciphertext. `NoCredential`, `ArchiveDamaged`, `Unsupported` and `BudgetExhausted` remain separate non-candidate results. `confirm_first` continues only after `NotAccepted`; it returns a non-candidate result as an error rather than reporting search exhaustion.

## Budgets and limits

The default per-entry materialization and solid preceding-byte budgets are each 64 MiB. `max_preceding_bytes: Some(0)` forbids preceding decoding; `None` explicitly permits unbounded preceding work. The wrapper never passes zero to the lower-level API, where zero disables that limit. `max_entry_bytes` checks the selected entry before extraction.

These are byte/work bounds, not a hard resident-memory guarantee for every codec and not an interruptible KDF or a wall-clock deadline. Archive parsing, ciphertext access, decoder internals and key derivation retain their existing implementation limits. Caller cancellation and final event publication are handled by the later engine/TUI stages.

## Implementation and regression evidence

- Model/selection: [target.rs](products/ttzip/core/rust/crates/ttzip-archive/src/recovery/target.rs#L1-L365), [verdict.rs](products/ttzip/core/rust/crates/ttzip-archive/src/recovery/verdict.rs#L1-L71).
- Production confirmation/search convenience API: [confirm.rs](products/ttzip/core/rust/crates/ttzip-archive/src/recovery/confirm.rs#L1-L352).
- Deterministic fixture provenance: [tests_support.rs](products/ttzip/core/rust/crates/ttzip-archive/src/recovery/tests_support.rs#L1-L383). Fixed synthetic plaintext, salts/IV/KDF parameters and the archive/crypto writers build the test archives in memory; no user media, external downloads or new stored fixture files are required.
- ZIP regressions: [tests_zip_confirmation.rs](products/ttzip/core/rust/crates/ttzip-archive/src/recovery/tests_zip_confirmation.rs#L1-L537), including a pinned screening false positive followed by a true candidate, valid zero CRC, mixed ordinary/encrypted entries, empty-entry and multi-password boundaries.
- 7z/AE-2 regressions: [tests_sevenz_confirmation.rs](products/ttzip/core/rust/crates/ttzip-archive/src/recovery/tests_sevenz_confirmation.rs#L1-L124), plus [tests_search_semantics.rs](products/ttzip/core/rust/crates/ttzip-archive/src/recovery/tests_search_semantics.rs#L1-L105) for non-candidate stop results and solid preceding-byte limits.

The existing BUILD source glob and test crate include all eight new Rust files. No new Cargo/Bazel dependency or bounded-7z API change is needed. Formal target: `//products/ttzip/core/rust/crates/ttzip-archive:ttzip_archive_test`; lint the exact archive crate as a separate serial gate. Historical mixed-tree pass counts do not constitute acceptance of this candidate.

## Subsequent atomic stages

Stage 2 adds engine `RecoverySearch` and candidate iteration, then routes the actual UniFFI recovery function and two archive-level C ABI recovery starts through screening followed by confirmation. Preserve existing ABI/record shapes and UniFFI documentation metadata/checksums. Non-candidate reasons must not become success/exhaustion; actual attempt counters and return-boundary cancellation arbitration belong to this stage. The crypto-only screening exports retain their existing low-level meaning.

Stage 3 jointly replaces TUI worker and CLI recovery implementations and migrates MM36 controlled cancellation tests. The engine return boundary and the TUI event commit are separate ownership boundaries: preserve the sole terminal publisher, pre/post-computation cancellation checks, no progress after terminal, sender release and no UI-thread join. Production startup, candidate generation and the real worker/event path must all use the confirmed pipeline. Freeze both engine and archive predecessors before validating this stage.

Neither stage may import unrelated DMG dependencies, MM45 reader follow-up, benchmark changes or broad live BUILD modifications to achieve compilation.

Stage 2 was independently accepted on committed archive stage 1 `3ccc5ac7b9c0d480eb29dcfd37bb1bcbe78342a1`; the archive predecessor and engine successor retain separate formal evidence.

## Stage 2: confirmed engine orchestration

`ttzip-engine::recovery::RecoverySearch` owns batching and actual attempt metrics. It prepares the archive confirmation once, screens ZipCrypto/AES candidates in bounded batches, confirms every survivor, continues after `NotAccepted`, and preserves non-candidate stop reasons. 7z has no cheap fixed-magic screen, so it counts actual content confirmations. ZIP counts actual screened candidates rather than the unvisited dictionary length.

The real UniFFI `recover_archive_password`, C ABI `ttzip_rust_password_recovery_start_dictionary`, and C ABI `ttzip_rust_password_recovery_start_brute_force` use this pipeline. Existing public signatures and result layouts remain unchanged. On ordinary candidate exhaustion, both archive-level C starters preserve `ErrInvalidPassword` and actual attempts, and reuse `write_out_string(None, ...)` to clear a non-null, positive-capacity output buffer. This restores prior no-result behavior for a reused buffer containing an old password; cancellation, preparation/stop errors and other output policies are unchanged. The real dictionary and brute-force exhaustion regressions prefill a NUL-terminated old password, rather than relying on an initially zero buffer. The original exported UniFFI documentation metadata is preserved; new implementation explanations are ordinary comments. The separate low-level crypto screening exports retain their screening-only meaning.

The engine's synchronous return boundary arbitrates cancellation against pending confirmed/exhausted results once; genuine non-candidate failures keep their meaning. This is not a guarantee of interruptible KDF or a hard wall-clock deadline. The later TUI event commit is a separate boundary and is not completed by this stage.

Tests: 28 new engine unit cases (16 search/candidate-source and 12 actual public-entry cases), including deterministic screening bait before the true password, bait-only exhaustion, AE-2, 7z, budget/credential/format/container stops, return-before-exhaustion cancellation, actual ZIP/7z counts, both archive-level C ABI starters, and candidate batching across length boundaries. The default synthetic fixture bytes are preserved; the helper also accepts a small owned password for the true brute-force entry regression. All tests are registered through existing module declarations and BUILD source globs; no stored fixture, dependency or BUILD change is needed.

Formal targets for this exact successor: `//products/ttzip/core/rust/ttzip-engine:ttzip_engine_test`, `//products/ttzip/core/rust/ttzip-engine:uniffi_checksum_gate_test`, and the predecessor archive unit target, plus strict engine lint. Root formal acceptance: engine 301 passed, archive 328 passed, UniFFI checksum 3 passed, plus strict offline `cargo clippy -p ttzip-engine --lib --tests -- -D warnings`. Removing only the two exhaustion-output clear operations makes exactly the two prefilled public-entry regressions fail; restoring them returns all three complete formal targets to green. Scoped quality and doc-twin checks passed. Historical mixed-tree results are not current acceptance. Stage 3 TUI/CLI consumption remains a separate dependent atomic change.
