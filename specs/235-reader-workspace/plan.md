# 实施计划

1. 新增 app-core reader 模块与 UniFFI Object，按稳定 identity 哈希分片原子存储 JSON。
2. 新增 EPUB reader adapter，产品宿主注入 Rust 回调，TTMediaKit 不依赖产品模块。
3. 修正分页手势/边界，新增中文/英文字体、选区与注释界面，连接 adapter。
4. root 注册 Rust 模块、接生产 adapter 与统一窗口，集中跑正式 Bazel 目标。

文件边界：infra/ttmedia 的 UI/Document/EPUB* 与专用 EPUB tests；app-core/src/reader/*；本 spec 与 preview reader 子孪生。共享 BUILD/lib.rs/路由由 root 独占。
