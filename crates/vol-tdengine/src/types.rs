//! TDengine response types.

use serde::Deserialize;
use serde_json::Value;

/// TDengine response format (TDengine 3.x REST API)
#[derive(Debug, Deserialize)]
pub struct TdengineResponse {
    pub code: i32,
    #[serde(default)]
    pub desc: Option<String>,
    #[serde(default)]
    pub data: Option<Value>,
    #[serde(default)]
    pub column_meta: Option<Value>,
    #[serde(default)]
    pub rows: Option<u32>,
}
