# 技术答辩与设计分析：磁盘空间预分配 (PR #3393 / Issue #3392)

> 对应工件：`specs/035-upstream-contribution-guardrails-and-pr-remediation/plan.md`  
> 针对 Reviewer @kientzle 提问的技术事实分析与设计演进方案

---

## 1. 关于“稀疏文件 (Sparse Files) 检测”的深度事实分析

### 1.1 Maintainer 提问
> **@kientzle**: *"Are you sure that `archive_write_disk_posix` can actually tell whether a file is sparse or not? For tar, at least, the sparse file information is stored in the header, but standard archive formats don't record whether a file was sparse on the original filesystem."*

### 1.2 源码事实与技术结论
经过深度分析 `archive_write_disk_posix.c`（第 984~1030 行），发现 libarchive 对稀疏文件的处理机制如下：

1. **动态穿透稀疏化 (Dynamic Sparsification)**：
   - 当调用方设置了 `ARCHIVE_EXTRACT_SPARSE` 标志时，`archive_write_disk` **并不依赖归档格式本身的头部元数据**。
   - 在 `write_data_block()` 热循环中（第 1011 行），`archive_write_disk` 会逐块扫描写入的缓冲区分块：
     ```c
     /* Skip leading zero bytes. */
     for (p = buff, end = buff + size; p < end; ++p) {
         if (*p != '\0')
             break;
     }
     a->offset += p - buff;
     ```
   - 一旦遇到全零块，它会直接推进 `a->offset` 并执行 `lseek(a->fd, a->offset, SEEK_SET)`，通过文件系统打洞（Hole Punching）来动态制造稀疏文件。

2. **预分配与稀疏文件的互斥关系**：
   - `F_PREALLOCATE` (macOS) 和 `posix_fallocate` (Linux) 会在底层为整个 `filesize` 分配实际物理块。
   - **如果同时启用了预分配与动态稀疏化，预分配会提前占满物理扇区，彻底使后续的 `lseek` 打洞优化失效**。
   - **设计结论**：当 `(a->flags & ARCHIVE_EXTRACT_SPARSE)` 为真时，`archive_write_disk` **必须自动禁用预分配**。

---

## 2. 关于“小文件自动跳过 vs 独立 Opt-in Flag”的设计共识

### 2.1 Maintainer 提问
> **@kientzle**: *"Your argument for having a separate opt-in flag instead of always pre-allocating is not convincing. `archive_write_disk` could just not pre-allocate for small files to avoid the extra syscall overhead."*

### 2.2 设计权衡对比

| 方案 | 优点 | 缺点 | 适用场景 |
| :--- | :--- | :--- | :--- |
| **方案 A：纯 Opt-in Flag (`ARCHIVE_EXTRACT_PREALLOCATE`)** | 显式控制，零默认行为改变 | 调用方需手动开启；解压数万小文件时若盲目开启会有额外 syscall 开销 | 明确知道要解压超大镜像/虚拟磁盘文件的专业工具 |
| **方案 B：阈值自动旁路 + Opt-in (`filesize >= 64KB`)** | 消除小文件无谓系统调用，兼顾大文件防碎片与连续盘块分配 | 增加了默认行为的细微逻辑分支 | **最优推荐方案** |

### 2.3 建议的演进代码设计

```c
/* 自动旁路：仅当非稀疏、文件大小已知且 >= 64KB 时执行预分配 */
if ((a->flags & ARCHIVE_EXTRACT_PREALLOCATE) &&
    !(a->flags & ARCHIVE_EXTRACT_SPARSE) &&
    a->filesize >= 65536) {
    /* 执行 Darwin F_PREALLOCATE 或 Linux posix_fallocate */
}
```

---

## 3. 社区沟通建议回复草案 (沉淀待用)

```markdown
Thank you @kientzle for the insightful points!

1. **Regarding Sparse Files**:
You are completely right. Standard archive formats do not store filesystem sparsity. In `archive_write_disk_posix.c` (lines 984-1030), libarchive implements dynamic sparsification by scanning zero bytes during write and seeking over them when `ARCHIVE_EXTRACT_SPARSE` is set.
Pre-allocating physical disk blocks upfront directly conflicts with dynamic sparsification because allocating physical extents prevents hole-punching. Therefore, we will ensure pre-allocation is automatically bypassed whenever `ARCHIVE_EXTRACT_SPARSE` is active.

2. **Regarding Small Files & Syscall Overhead**:
Agreed. For small files (< 64KB), the syscall overhead of `fcntl(F_PREALLOCATE)` or `posix_fallocate` exceeds any defragmentation benefit. We can add a minimum size threshold (e.g., >= 64KB) so small files are automatically skipped even when pre-allocation is requested.

We will keep this PR in draft while discussion continues on #3392.
```
