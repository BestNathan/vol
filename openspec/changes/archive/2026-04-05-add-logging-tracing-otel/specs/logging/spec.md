## ADDED Requirements

### Requirement: 控制台日志输出

系统应当将日志输出到控制台（stdout），使用紧凑格式，支持颜色高亮，日志级别可配置。

#### Scenario: INFO 级别日志输出
- **WHEN** 代码调用 `tracing::info!("Starting monitor")`
- **THEN** 控制台输出带时间戳和级别的日志，如 `01:23:45.123 INFO [vol_monitor] Starting monitor`

#### Scenario: 颜色高亮
- **WHEN** 日志输出到支持 ANSI 颜色的终端
- **THEN** ERROR 显示红色，WARN 显示黄色，INFO 显示绿色，DEBUG 显示灰色

#### Scenario: 日志级别过滤
- **WHEN** 配置 `console_level = "warn"`
- **THEN** 控制台仅显示 WARN 和 ERROR 级别日志，INFO 和 DEBUG 被过滤

### Requirement: 文件日志输出

系统应当将日志输出到文件，支持按天滚动，自动删除过期文件，使用 JSON 格式。

#### Scenario: 日志文件命名
- **WHEN** 配置 `log_prefix = "vol-monitor"`
- **THEN** 日志文件名为 `vol-monitor-2026-04-05.log`

#### Scenario: 按天滚动
- **WHEN** 日期从 2026-04-05 变为 2026-04-06
- **THEN** 新日志写入 `vol-monitor-2026-04-06.log`，旧文件保持不变

#### Scenario: 自动删除过期文件
- **WHEN** 配置 `retention_days = 7`，当前日期为 2026-04-15
- **THEN** `vol-monitor-2026-04-07.log` 及更早的文件被自动删除

#### Scenario: JSON 格式输出
- **WHEN** 配置 `json_format = true`
- **THEN** 日志文件每行是一个 JSON 对象，包含 timestamp、level、target、message 字段

### Requirement: 错误日志独立文件

系统应当将 ERROR 级别日志同时输出到独立的错误日志文件，便于快速定位问题。

#### Scenario: 错误日志命名
- **WHEN** 配置 `log_prefix = "vol-monitor"`
- **THEN** 错误日志文件名为 `vol-monitor-error-2026-04-05.log`

#### Scenario: 仅 ERROR 级别
- **WHEN** 代码调用 `tracing::warn!("Connection lost")`
- **THEN** 警告日志不写入错误文件，仅普通日志文件

#### Scenario: 错误日志包含完整上下文
- **WHEN** 代码调用 `tracing::error!("Failed: {}", e)` 且当前 span 有 trace_id
- **THEN** 错误日志 JSON 包含 `span` 字段，内有 trace_id 和 span 名称

### Requirement: 日志配置化

所有日志相关参数应当可通过 `config.toml` 配置，并支持环境变量覆盖。

#### Scenario: 配置日志目录
- **WHEN** 配置 `log_dir = "/var/log/vol-monitor"`
- **THEN** 所有日志文件写入 `/var/log/vol-monitor/` 目录

#### Scenario: 环境变量覆盖
- **WHEN** 配置 `log_dir = "logs"` 且环境变量 `LOG_DIR = "/tmp/logs"`
- **THEN** 实际使用 `/tmp/logs` 作为日志目录

#### Scenario: 禁用错误文件
- **WHEN** 配置 `error_file = false`
- **THEN** 不创建错误日志文件，ERROR 日志仅输出到普通日志文件
