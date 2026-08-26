# Data Model & Manifest Entities: TTZip 全语言 SDK 零配置分发与外部开发者易用性体系

- **Feature ID**: `007-zero-config-sdk-distribution-and-developer-experience`
- **Date**: 2026-08-24

---

## 1. 实体与模型定义 (Entities & Schemas)

### 1.1 `PlatformClassifier` (平台分类器实体)
用于跨语言原生动态库寻址与分类：

```typescript
interface PlatformClassifier {
  os: "darwin" | "linux" | "windows";
  arch: "aarch64" | "x86_64" | "arm64" | "x64";
  classifier: string;          // e.g. "darwin-aarch64", "linux-x86_64", "windows-x86_64"
  libraryFileName: string;     // e.g. "libttzip_engine.dylib", "libttzip_engine.so", "ttzip_engine.dll"
  targetTriple: string;        // e.g. "aarch64-apple-darwin", "x86_64-unknown-linux-gnu"
}
```

### 1.2 `NativeLoaderDescriptor` (原生加载器自提取描述符)
描述运行时动态库提取与缓存状态：

```typescript
interface NativeLoaderDescriptor {
  version: string;
  sourceType: "system_property" | "env_variable" | "embedded_jar_resource" | "dev_workspace" | "system_path";
  resolvedPath: string;
  sha256Checksum: string;
  loadDurationMs: number;
  isCached: boolean;
  status: "LOADED" | "FAILED";
  diagnosticsLog: string[];
}
```

### 1.3 `CMakeTargetTopology` (CMake 目标拓扑实体)
描述 C/C++ 导出的现代目标与其传递依赖：

```typescript
interface CMakeTargetTopology {
  cppTarget: "ttzip::ttzip_cpp";
  cppCompileFeature: "cxx_std_20";
  cppType: "INTERFACE";
  
  cTarget: "ttzip::ttzip_c";
  cCompileFeature: "c_std_11";
  cType: "STATIC" | "INTERFACE";
  cLocation: string;           // Path to libttzip_engine.a
  
  transitiveLibraries: string[]; // ["Threads::Threads", "archive", "bz2", "z", "lzma", "Security", "CoreFoundation"]
  pkgConfigPrivateLibs: string;  // "-larchive -lbz2 -lz -llzma -lpthread -framework Security -framework CoreFoundation"
}
```

### 1.4 `OutOfTreeSmokeResult` (纯净容器冒烟测试报告)
描述在独立干净环境中测试各语言 Quickstart 的运行结果：

```typescript
interface OutOfTreeSmokeResult {
  schemaVersion: "1.0.0";
  timestamp: string;
  isolatedTestDir: string;
  results: {
    language: string;          // "java" | "python" | "cpp" | "c" | "go" | "dart" | "dotnet"
    artifactType: string;      // "jar" | "wheel" | "cmake_pkg" | "go_module" | "pub_pkg" | "nuget_pkg"
    buildDurationSeconds: number;
    executionDurationSeconds: number;
    exitCode: number;
    outputSample: string;
    passed: boolean;
  }[];
  allPassed: boolean;
}
```
