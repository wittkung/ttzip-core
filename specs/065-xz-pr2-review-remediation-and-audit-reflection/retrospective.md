# Root Cause Analysis & Upstream Audit Retrospective

## 一、 审查问题归因与根因复盘 (Root Cause Analysis)

### 1. 移位注释左右方向颠倒（`shift_left` 注释写成 "Shift right"）
- **触发根因**：
  - 代码在架构上移植自 `crc_x86_clmul.h`。在 x86 SSSE3 / AVX 向量生态中，`_mm_slli_si128` 与 `_mm_srli_si128` 的字节序命名在历史上与内存地址的高低位关系常常导致认知混淆。
  - 在将 x86 C 语言注释迁移至 ARM64 NEON 时，由于函数名（`shift_left`/`shift_right`）与向量内联汇编/查找表（`vqtbl1q_u8`）本身逻辑正确并通过了单测，审计者只关注了代码逻辑与寄存器行为，未能逐字对齐自然语言注释中的 "left/right" 语义。
- **为什么自动化单测没拦截出来？**：
  - 编译器和单测套件（CTest / ASan）只执行机器码，无法感知代码上方注释的英语方向词是否与函数名一致。

### 2. `keep_high_bytes` 注释与代码不匹配（注释写了 "Shift the bytes..."）
- **触发根因**：
  - 在 x86 的原始实现中，处理尾部非对齐字节是通过移位（shift）实现的；而在 ARM64 的特化优化中，我们改用了直接查表与按位与（`vandq_u8(v, vld1q_u8(vmasks + count))`）以达到更高的单周期吞吐。
  - 重构代码时，算法实现被成功替换为 `vandq_u8` 掩码清零，但上方的多行解释注释仍然保留了 x86 版本的 "Shift the bytes so that the last size bytes are at high bits..."。

### 3. macOS `is_arch_extension_supported` 的布尔回退漏洞（`return true;` 无条件兜底）
- **触发根因**：
  - 在编写 Darwin `sysctlbyname` 分支时，写出了如下结构：
    ```c
    if (sysctlbyname("hw.optional.arm.FEAT_PMULL", &has_pmull, &size, NULL, 0) == 0 && has_pmull)
        return true;
    return true; // ⚠️ 原本意图是作为静态兜底，但写在了 sysctlbyname 判定之后，导致 false 条件被穿透
    ```
- **为什么在本地测试全绿？**：
  - 本地运行环境是真实的 Apple Silicon 硬件（Apple M5 Max / M1 / M2），在此硬件上 `sysctlbyname("hw.optional.arm.FEAT_PMULL")` 返回的 `has_pmull` **永远为 1**。
  - 在真实硬件上永远只走 `if (...) return true;` 分支，导致后方的 `return true;` 兜底代码成为了永远无法被测试用例覆盖到的“死代码（Dead Fallthrough）”。

---

## 二、 流程防线与工作流改进 (Preventative Process Guardrails)

针对上述 3 处审计盲区，确立以下系统级改进铁律：

1. **【注释-代码双向语义核验铁律 (Comment-Code Invariant Sync)】**：
   - 凡从既有模块（如 x86）移植/重构算法并修改实现细节时，**必须同时执行注释语义清理**。禁止保留原有的操作动词（如 "Shift"），必须如实陈述实际机器操作（如 "Clear lowest bytes via bitwise AND"）。
2. **【分支穿透与静态死路径审查 (Branch Fallthrough Audit)】**：
   - 任何运行时特性探测函数（`is_arch_extension_supported`），返回值必须直接绑定探测结果（如 `return has_pmull != 0;`），绝对禁止在探测失败路径上残留硬编码的 `return true;`。
3. **【可复现性前置交付 (Reproducibility First)】**：
   - 任何涉及硬件吞吐声明的 PR，必须附带单文件零依赖的独立 C 语言复现脚本，提供一行命令即可验证的科学环境。
