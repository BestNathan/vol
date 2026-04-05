## 1. vol-tracing 工具库增强

- [ ] 1.1 添加 `uuid` 依赖到 `vol-tracing/Cargo.toml`
- [ ] 1.2 实现 `new_trace_id()` 函数（UUID v4）
- [ ] 1.3 实现 `current_trace_id()` 函数（从 span context 提取）
- [ ] 1.4 Re-export `tracing::Instrument` trait
- [ ] 1.5 Re-export `tracing::instrument` 宏
- [ ] 1.6 运行 `cargo check -p vol-tracing` 验证编译

## 2. vol-datasource 模块重构

- [ ] 2.1 修改 `volatility.rs`：send 操作改用 `.instrument()`
- [ ] 2.2 修改 `portfolio.rs`：send 操作改用 `.instrument()`
- [ ] 2.3 更新 import：添加 `use vol_tracing::Instrument`
- [ ] 2.4 运行 `cargo check -p vol-datasource` 验证

## 3. vol-engine 模块重构

- [ ] 3.1 修改 `engine.rs`：`rule.evaluate()` 使用 `.instrument()`
- [ ] 3.2 清理未使用的 `record_tags` import
- [ ] 3.3 运行 `cargo check -p vol-engine` 验证

## 4. vol-notification 模块重构

- [ ] 4.1 修改 `feishu.rs`：`send_message()` 使用 `.instrument()`
- [ ] 4.2 修改 `stdout.rs`：保持 `.enter()`（同步操作）
- [ ] 4.3 运行 `cargo check -p vol-notification` 验证

## 5. 全 Workspace 验证

- [ ] 5.1 运行 `cargo check --workspace`
- [ ] 5.2 运行 `cargo test --workspace`（如有测试）
- [ ] 5.3 运行 `cargo build --release` 验证发布构建

## 6. 文档更新

- [ ] 6.1 更新 `docs/tracing.md` 添加 `.instrument()` 使用示例
- [ ] 6.2 添加 Span 命名规范文档
- [ ] 6.3 添加跨 channel 传播模式示例
