# Implementation Plan: Historical Peak Gap Closure & Unified Fast-Path Alignment (Feature 019)

**Feature**: Historical Peak Gap Closure & Unified Fast-Path Alignment  
**Directory**: `specs/019-historical-peak-gap-closure-and-unified-fast-path/`  
**Status**: Ready for Tasks

---

## 1. Technical Context & Overview

根据历史峰值矩阵诊断结果，优化两处核心架构瓶颈：
1. **分发层目录快速路径解除限制**：在 `ArchiveWriter+Dispatch.swift` 中，对于目录输入，若总字节数 $< 500\text{MB}$，直接分发进入 `ttzip_create_archive_tuned` / `ttzip_create_tar_native_c`，彻底打通海量小文件目录打包的高速公路。
2. **轻量信息熵短路**：在 `NativeCoreArchitecture` / `ArchiveWriter+Dispatch` 中提供 `isHighEntropyPayload` 探测方法，抽样头部 64KB 计算香农熵，对高熵不可压缩数据将 Level 降至 1，避免 CPU 字典搜索空转。

---

## 2. Component Modifications

### 2.1 Swift 归档分发层路由与高熵探测
- **文件**: `Sources/TTZipCore/ArchiveWriter+Dispatch.swift`
- **改动**: 
  1. 移除 `!hasDirectoryInput` 对 `ttzip_create_archive_tuned` 的阻断，使目录输入也能享受 C 层极速流式打包。
  2. 针对高熵 Payload，自动使用最轻量压缩等级，避免 Deflate / LZMA2 空转。

### 2.2 C 桥接层目录扫描与批量写入
- **文件**: `Sources/CTTZipBridge/ttzip_tar_native.c`
- **改动**: 优化 `ttzip_create_tar_native_c` 中的 `write_reg_file_data` 缓冲区调度，确保多核与页缓存充分命中。
