# Quickstart & Verification Guide: 上游开源贡献质量规范体系与 3 个 PR 严谨重构

> 对应工件：`specs/035-upstream-contribution-guardrails-and-pr-remediation/plan.md`

---

## 验证场景 1：全局 Agent 规则与审查规范落地验证

### Command
```bash
# 验证 code-review 与 upstream-contribution 规范存在且包含系统级 C 防御章节
grep -n "跨平台防御性编程" /Users/kevintung/.agents/skills/code-review/SKILL.md
test -f /Users/kevintung/.agents/skills/upstream-contribution/SKILL.md && echo "Upstream contribution skill exists"
```

### Expected Output
- 匹配到《系统级 C / 跨平台防御性编程审查规范》章节与检查点。
- 输出 `Upstream contribution skill exists`。

### Failure Diagnostic
- 若未找到匹配，检查 `SKILL.md` 是否已正确保存或路径是否准确。

---

## 验证场景 2：PR #3391 (CRC32) 纯净分支与原生预言机验证

### Command
```bash
cd /Users/kevintung/Documents/dev/TTZip/Vendor/libarchive-upstream
git checkout armv8-crc32-acceleration
# 物理断言：严格检查修改文件仅有 4 个
git diff origin/master..HEAD --stat
# 运行 CRC32 单元测试
mkdir -p build && cd build && cmake .. -DENABLE_ZLIB=OFF -DENABLE_TEST=ON && make -j8
./libarchive/test/libarchive_test -r ../ test_archive_crc32
```

### Expected Output
- `git diff origin/master..HEAD --stat` 仅显示：
  - `libarchive/archive_crc32.h`
  - `libarchive/test/test_archive_crc32.c`
  - `libarchive/test/CMakeLists.txt`
  - `Makefile.am`
- 零 7z 或 cryptor 相关文件。
- `test_archive_crc32` 5 组测试用例（包括 `bitcrc32` 交叉校验）全部 `Passed`。

### Failure Diagnostic
- 若存在无关文件，运行 `git diff --stat origin/master` 排查未剥离的 commit。

---

## 验证场景 3：PR #3388 (7z AES) 原子 Commit 与 32 位截断/流式判空验证

### Command
```bash
cd /Users/kevintung/Documents/dev/TTZip/Vendor/libarchive-upstream
git checkout feat/7z-aes256-decryption
# 验证 commit 序列呈现清晰的 3 个原子提交
git log origin/master..HEAD --oneline
# 运行 7z 加密回归测试
cd build && make -j8
./libarchive/test/libarchive_test -r ../ test_read_format_7zip_encryption_data
./libarchive/test/libarchive_test -r ../ test_read_format_7zip_encryption_header
./libarchive/test/libarchive_test -r ../ test_read_format_7zip_encryption_partially
```

### Expected Output
- `git log` 显示 3 个独立 commit：`[infra] cryptor` -> `[feat] 7zip` -> `[test] test_read_format_7zip`。
- 3 组加密解包测试全部返回 `Passed`。

### Failure Diagnostic
- 若解密测试失败，检查 `extract_pack_stream` 中的 `read_ahead` 判空与 clamp 逻辑是否正确。
