# Data Model: TTZip 工业级测试体系 (Feature 169)

**Feature ID**: `169-rust-industrial-test-and-fuzz-suite`  
**Created**: 2026-08-21  
**Status**: Completed  
**Artifact**: Phase 1 Data Model & Types

---

## 1. 测试实体与配置模型

### 1.1 属性测试生成参数 (Property Generation Model)

```rust
#[derive(Debug, Clone)]
pub struct PropertyArchiveTreeSpec {
    pub root_dir_name: String,
    pub entries: Vec<PropertyEntrySpec>,
    pub compression_level: u32,
    pub encryption_method: u32,
    pub password: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PropertyEntrySpec {
    pub relative_path: String,
    pub payload_bytes: Vec<u8>,
    pub is_directory: bool,
    pub unix_mode: u32,
}
```

### 1.2 变异 Fuzzing 统计模型 (Fuzz Mutation Statistics)

```rust
#[derive(Debug, Clone)]
pub struct FuzzExecutionReport {
    pub target_name: String,
    pub iterations_completed: u64,
    pub invalid_inputs_gracefully_rejected: u64,
    pub panics_count: u64,
    pub max_resident_memory_kb: u64,
}
```

### 1.3 纳秒级微基准结果模型 (Benchmark Result Model)

```rust
#[derive(Debug, Clone)]
pub struct BenchmarkPointResult {
    pub benchmark_name: String,
    pub payload_size_bytes: usize,
    pub mean_duration_nanos: f64,
    pub throughput_mb_per_sec: f64,
    pub standard_deviation_percent: f64,
}
```
