# Log File Naming Convention Design

## Overview

Change log file naming convention from `prefix.log.YYYY-MM-DD` to `prefix-YYYY-MM-DD.log` format.

## Current Behavior

- Regular log: `vol-monitor.log.2026-04-05`
- Error log: `vol-monitor.error.log.2026-04-05`

This is the default `tracing-appender` behavior where the date suffix is appended after the full filename.

## Desired Behavior

- Regular log: `vol-monitor-2026-04-05.log`
- Error log: `vol-monitor-2026-04-05.error.log`

Date is inserted between the prefix and the file extension.

## Implementation

Modify `crates/vol-monitor/src/tracing_setup.rs`:

```rust
// Before
fn create_file_appender(config: &LoggingConfig) -> RollingFileAppender {
    RollingFileAppender::new(
        Rotation::DAILY,
        &config.log_dir,
        format!("{}.log", config.log_prefix),
    )
}

// After - date prefix format
fn create_file_appender(config: &LoggingConfig) -> RollingFileAppender {
    RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(format!("{}", config.log_prefix))
        .filename_suffix("log")
        .build(&config.log_dir)
}
```

Note: `tracing-appender` 0.2+ supports `builder()` API with `filename_prefix` and `filename_suffix` for custom naming.

## Benefits

1. **Natural sorting** - Files sort chronologically by name
2. **Standard convention** - Matches nginx, redis, and other common tools
3. **Easier globbing** - Pattern `vol-monitor-*.log` matches all dated files

## Trade-offs

- Requires `tracing-appender` builder API (slightly more verbose)
- Existing log files will have mixed naming (minor issue)

## Testing

1. Run application and verify log file naming
2. Verify daily rotation creates correctly named files
3. Verify error log file uses same naming pattern
