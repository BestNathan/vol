//! vol-feishu: Feishu/Lark API client.
//!
//! Implements OAuth 2.0 authentication and message sending.
//! Reference: https://open.feishu.cn/document/server-docs/api-call-guide/calling-process/get-access-token

pub mod client;

pub use client::{FeishuClient, FeishuError};
