# Data Model: 182-sink-benchmark-pareto-concurrency-and-cli-to-rust

## 1. Pareto Frontier Models (`rust/ttzip-glue/src/benchmark/pareto.rs`)
- **`ParetoPoint`**:
  - `codec_name: String`
  - `compression_ratio: f64`
  - `speed_mb_per_sec: f64`
  - `is_pareto_optimal: bool`

## 2. Secure Vault Models (`rust/ttzip-glue/src/crypto/vault.rs`)
- **`VaultItem`**:
  - `key_id: String`
  - `ciphertext: Vec<u8>`
  - `nonce: [u8; 12]`
  - `salt: [u8; 16]`
