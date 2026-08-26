# Quickstart & Verification Guide: TTZip 全语言 SDK 零配置分发与外部开发者极致易用性体系

- **Feature ID**: `007-zero-config-sdk-distribution-and-developer-experience`
- **Date**: 2026-08-24

---

## 1. 验证目标 (Validation Objectives)

证明以下 5 大核心场景在**完全脱离源码仓库的纯净空白目录**中能够零配置直接运行：
1. **Java 22+ & Kotlin**: 零 JVM 参数 (`-Dttzip.lib.path`) 自动提取运行。
2. **Python**: 预编译 ABI3 Wheel 安装后运行 `quickstart.py`。
3. **C++20 & C11**: CMake `find_package(ttzip REQUIRED)` 自动链接传递依赖。
4. **Go**: CGO 自包含头文件与静态库编译运行。
5. **CI 门禁**: `make -C core test-out-of-tree-smoke` 在 15 秒内全部通过。

---

## 2. 自动化验证命令 (Automated Smoke Commands)

```bash
# 1. 运行 Out-Of-Tree 纯净容器冒烟测试套件
bash core/scripts/run_out_of_tree_smoke.sh

# 2. 验证 Java 22+ Panama FFM 零配置加载
java --enable-preview -cp core/sdk/jvm/bin com.ttzip.NativeLoaderTest

# 3. 验证 Python ABI3 Wheel 打包与类型检查
cd core && python3 -m maturin build --release
python3 -c "import ttzip; print(ttzip.__version__, ttzip.is_hardware_accelerated())"
mypy python/ttzip/

# 4. 验证 C++20 CMake 现代目标集成
cmake -B /tmp/ttzip_cpp_smoke -S core/examples/cpp -DCMAKE_PREFIX_PATH=/tmp/ttzip_dist
cmake --build /tmp/ttzip_cpp_smoke
/tmp/ttzip_cpp_smoke/quickstart_cpp

# 5. 验证 Go 独立模块构建
cd core/examples/go && go run quickstart.go
```
