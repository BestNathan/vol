# Skill File Viewer Design

## Problem

SkillDetailDialog 的文件列表使用 `max-h-32` 固定高度，文件条目不可点击，无法查看文件内容。

## Solution

采用列表 + 内容区分离布局：

- **Dialog 尺寸**：`w-[800px] h-[80vh]`（更大以容纳双区域）
- **文件列表区**：固定 `max-h-[35%]` 可滚动，每行可点击
- **内容预览区**：`flex-1` 占剩余空间，可滚动 `<pre>` 格式

### 交互

1. 点击文件行 → 高亮该行 → 调用 `file_read` RPC → 内容区显示文件
2. 加载中显示 "Loading..." 占位
3. 未选文件时显示 "Click a file to preview" 占位

### 改动范围

仅修改 `skill_detail_dialog.rs`，复用已有的 `JsonRpcClient.file_read`。
