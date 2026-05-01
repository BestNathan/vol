# Requirements: md-frontmatter Crate

## Background
目前有两个地方在各自解析 YAML frontmatter：
- `vol-llm-skill/src/parser.rs` — 硬编码 `SkillFrontmatter` 结构体
- `vol-llm-wiki/src/loader.rs` — 手动字符串解析 title/tags

两者都缺乏行号错误定位、frontmatter 更新写入（保持 body 不变）、目录批量扫描等功能。提取为一个通用的 `md-frontmatter` crate 可以统一这两个实现，并提供更完整的 API。

## Goals
1. 提供通用的 YAML frontmatter 解析：不预设任何字段，通过 `serde` 反序列化到使用方提供的任意 struct
2. 提供文件级便捷 API：`from_path::<T>()`、`from_str()`
3. 支持目录批量扫描：发现所有 `.md` 文件，解析 frontmatter，返回结果集合
4. 支持 frontmatter 更新/写入：修改 frontmatter 后写回文件，body 部分逐字节保持不变
5. 解析错误带行号定位：类似编译器报错，指出 frontmatter 第几行出了什么问题

## Non-Goals
1. 不支持 TOML (`+++`) 或 JSON (`{...}`) frontmatter — 只支持 YAML (`---`)
2. 不支持大文件 lazy/stream 模式 — 假设文档不会特别大，可以一次性加载
3. 不提供 markdown body 的内容解析（如按标题切分 sections）— 只负责分离 frontmatter 和 body

## Scope

### Included
- `---` 分隔符识别和 YAML frontmatter 提取
- `serde_yaml` 反序列化为使用方的任意 `T: DeserializeOwned`
- 文件读取便捷方法
- 目录递归扫描 + 批量解析
- Frontmatter 修改后写回（body 不变）
- 带行号的错误类型

### Excluded
- TOML/JSON frontmatter 格式
- 大文件的 peek-first-then-read-body 模式
- Markdown AST 解析
- 变更检测/哈希记录

## Constraints
- 命名: `md-frontmatter`
- 依赖: `serde`, `serde_yaml`, `thiserror`（错误处理）, `glob` 或 `walkdir`（目录扫描）
- 纯同步 API，不需要 async
- 遵循 workspace 的 `serde` 和 `thiserror` 版本

## Success Criteria
1. 解析正确: 能通过所有 parser 单元测试（含有效/无效/缺失 frontmatter 边界情况）
2. 错误行号: 解析失败时错误信息包含 frontmatter 内的行号（如 "line 3: ..."）
3. Body 不变: 修改 frontmatter 后写回，body 部分与原文件逐字节相同
4. 替换现有实现: `vol-llm-skill` 和 `vol-llm-wiki` 的 frontmatter 解析代码可以用 `md-frontmatter` 一行替换

## Edge Cases
1. **缺失 frontmatter**: 文件没有 `---` 分隔符 — 返回空 frontmatter（使用 T 的默认值）+ 完整内容作为 body
2. **无效 YAML**: 有 `---` 但内容不是合法 YAML — 返回带行号的解析错误
3. **前导空白**: 文件开头有空行/空白后再 `---` — 应正确识别
4. **只有 frontmatter 没有 body**: `---` 后立即结束或只有空行 — body 为空字符串
5. **body 中包含 `---`**: markdown body 中的水平线不应被误识别为 frontmatter 结束
6. **空目录**: 批量扫描空目录返回空列表
7. **非 .md 文件**: 批量扫描只解析 `.md` 扩展名的文件
8. **UTF-8 编码**: 只支持 UTF-8，非 UTF-8 文件返回错误

## Open Questions
无。
