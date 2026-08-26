# Data Model: 060-arm64-pmull-crc64-acceleration

**Feature Name**: ARM64 PMULL 硬件级 CRC64 (ECMA-182) 加速引擎接入  
**Status**: Modeled  
**Created**: 2026-08-17  
**Parent Plan**: [plan.md](./plan.md)

---

## 1. C-Level Interface Structures & Types

### 1.1 `ttzip_crc64` Signature Model
```c
uint64_t ttzip_crc64(const uint8_t *buf, size_t size, uint64_t crc);
```
- **Fields / Parameters**:
  - `buf`: `const uint8_t *` (nullable pointer to data buffer). If `NULL` or `size == 0`, operation is a no-op returning `crc`.
  - `size`: `size_t` (unsigned integer in range $[0, 2^{64}-1]$ representing the byte count).
  - `crc`: `uint64_t` (initial seed, default 0 for ECMA-182 standard).
- **Return Type**: `uint64_t` (computed 64-bit CRC value in inverted-in/inverted-out ECMA-182 format).

### 1.2 `ttzip_crc64_pmull` Signature Model
```c
uint64_t ttzip_crc64_pmull(const uint8_t *buf, size_t size, uint64_t crc);
```
- **Fields / Parameters**:
  - `buf`: `const uint8_t *` (buffer pointer).
  - `size`: `size_t` (byte length).
  - `crc`: `uint64_t` (initial seed).
- **Return Type**: `uint64_t` (computed 64-bit CRC value via ARM64 PMULL).

---

## 2. Swift-Level Interface Model

### 2.1 `CRC64Checksum` Type Definition
```swift
@frozen
public enum CRC64Checksum: Sendable {
    @inlinable
    public static func calculate(for data: Data, seed: UInt64 = 0) -> UInt64

    @inlinable
    public static func calculate(buffer: UnsafeRawBufferPointer, seed: UInt64 = 0) -> UInt64
}
```

- **Entities**:
  - `CRC64Checksum`: Static stateless namespace enum conforming to `Sendable`.
- **Validation Rules**:
  - If `data.isEmpty` -> returns `seed` immediately.
  - If `buffer.isEmpty` or `buffer.baseAddress == nil` -> returns `seed` immediately.
  - Non-empty buffers execute zero-copy borrowing through `withUnsafeBytes` / `baseAddress`.

---

## 3. Mathematical & Constants Model

| Constant Entity | Type | Value (Hex) | Purpose |
| :--- | :--- | :--- | :--- |
| `POLYNOMIAL_ECMA182_REFLECTED` | `uint64_t` | `0xC96C5795D7870F42` | ECMA-182 反转标准多项式 |
| `GOLDEN_ASCII_123456789` | `uint64_t` | `0x6C40DF5F0B497347` | ASCII `"123456789"` 黄金测试向量 |
| `BARRETT_FOLD512_HIGH` | `uint64_t` | `0x081F6054A7842DF4` | 512 位（64 字节）高阶折叠常数 $x^{576} \pmod P$ |
| `BARRETT_FOLD512_LOW` | `uint64_t` | `0x6AE3EFBB9DD441F3` | 512 位（64 字节）低阶折叠常数 $x^{512} \pmod P$ |
| `BARRETT_FOLD128_HIGH` | `uint64_t` | `0xDABE95AFC7875F40` | 128 位（16 字节）高阶折叠常数 $x^{192} \pmod P$ |
| `BARRETT_FOLD128_LOW` | `uint64_t` | `0xE05DD497CA393AE4` | 128 位（16 字节）低阶折叠常数 $x^{128} \pmod P$ |
| `BARRETT_MU_P_HIGH` | `uint64_t` | `0x9C3E466C172963D5` | Barrett 倒数多项式 $\mu = \lfloor x^{64} / P \rfloor$ |
| `BARRETT_MU_P_LOW` | `uint64_t` | `0x92D8AF2BAF0E1E84` | 模约化二次折叠多项式 $(P \ll 1) \mid 1$ |

---

## 4. Benchmark & Validation Metric Model

| Metric | Type | Unit | Floor / Target |
| :--- | :--- | :--- | :--- |
| `throughputMBps` | `Double` | MB/s | $\ge 30,000.0\text{ MB/s}$ |
| `goldenVectorExact` | `Boolean` | bool | `true` (`0x6C40DF5F0B497347`) |
| `differentialConsistency` | `Boolean` | bool | `true` (0~256 bytes & random chunks) |
