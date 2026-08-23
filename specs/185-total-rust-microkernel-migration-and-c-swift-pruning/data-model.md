# Data Model: 185-total-rust-microkernel-migration-and-c-swift-pruning

## 1. Password Recovery Models (`rust/ttzip-glue/src/crypto/password_recovery.rs`)
- **`PasswordRecoveryTarget`**:
  - `archive_path: PathBuf`
  - `encryption_type: ArchiveEncryptionType`
  - `salt_or_header: Vec<u8>`
  - `verification_bytes: [u8; 4]`

- **`RecoverySessionConfig`**:
  - `dictionary_words: Vec<String>`
  - `charset: String`
  - `max_length: usize`
  - `thread_count: usize`

- **`RecoveryProgressReport`**:
  - `attempts_count: u64`
  - `passwords_per_sec: f64`
  - `found_password: Option<String>`
