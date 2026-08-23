# Quickstart Guide: 7z Solid 解压与 ARM64 密码加速 (Feature 167)

## Scenario 1: 执行 7z 硬件 AES-256 与 SHA-256 单元测试
- **Command**:
  ```bash
  cmake --build build --target ttzip_c_test_runner && ./build/ttzip_c_test_runner 7z_crypto_neon
  ```
- **Expected Output**:
  - NIST FIPS-197 AES-256-CBC 向量校验 100% 通过
  - ARM64 硬件 SHA-256 KDF 2^19 轮派生与金标数据一致
  - 耗时在微秒级。

---

## Scenario 2: 运行 5 轮 Worktree A/B 基准对标
- **Command**:
  ```bash
  ./scripts/benchmark_ab.sh HEAD WIP --runs 5
  ```
- **Expected Output**:
  - 自动创建隔离工作区并编译
  - 输出 80+ 项指标统计对比表
  - 验证 0 项显著回退，保持 `PASSED_NO_REGRESSION`。
