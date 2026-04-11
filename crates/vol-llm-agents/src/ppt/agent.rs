//! PPT Agent 核心实现。

use crate::ppt::{PptAgentConfig, PptInput, PptOutput};

/// PPT Agent
pub struct PptAgent {
    config: PptAgentConfig,
}

impl PptAgent {
    pub async fn new(config: PptAgentConfig) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self { config })
    }

    pub async fn generate(&self, _input: PptInput) -> Result<PptOutput, Box<dyn std::error::Error>> {
        todo!("Implement PPT generation flow")
    }
}
