# Data Model: 黄金预言机、变异模糊测试与系统差分测试模型

**Feature Directory**: `specs/037-libarchive-golden-oracle-and-fuzz-integration`  
**Date**: 2026-08-16  
**Status**: Ready for Planning

---

## 1. 实体定义与字段规范

### 1.1 `TestingOracleSpec` (测试预言机与质量矩阵契约模型)
描述黄金语料库、变异模糊测试与系统差分验证的配置与门禁标准。

| 字段名 | 类型 | 必填 | 约束与说明 |
| :--- | :--- | :--- | :--- |
| `spec_version` | `string` | 是 | 规范版本，固定为 `"1.0.0"` |
| `uu_decoder` | `UUDecoderSpec` | 是 | UUDecode 解码器配置 |
| `fuzz_mutation` | `FuzzMutationSpec` | 是 | 变异模糊测试门禁配置 |
| `differential_oracle` | `DifferentialOracleSpec` | 是 | 系统 CLI 差分测试配置 |

- **`UUDecoderSpec`**:
  - `algorithm` (`string`, 必填): 解码算法，固定为 `"STANDARD_6BIT_TABLE_EXPANSION"`
  - `min_throughput_mb_s` (`number`, 必填): 最小解码吞吐量，固定为 `100.0`
- **`FuzzMutationSpec`**:
  - `mutation_ratio` (`number`, 必填): 变异字节比例，固定为 `0.01` (1%)
  - `iterations_per_run` (`integer`, 必填): 单次运行迭代次数，最小 `100`
  - `crash_dump_filename` (`string`, 必填): 崩溃转储文件名，固定为 `"fuzz_crash_reproducer.bin"`
  - `dual_mode_consumption` (`boolean`, 必填): 是否启用全解压与跳过双模式验证，固定为 `true`
- **`DifferentialOracleSpec`**:
  - `system_tools` (`Array<string>`, 必填): 差分对比的系统原生工具列表，如 `["/usr/bin/tar", "/usr/bin/unzip"]`
  - `byte_exact_sha256` (`boolean`, 必填): 是否进行 SHA-256 逐字节一致性断言，固定为 `true`
