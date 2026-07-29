/// Per-session capability adjustment, keyed by (agent_id, session_id).
/// Lives in AgentRuntime, purely in-memory. Survives frontend refresh,
/// dies on server restart.
#[derive(Debug, Clone)]
pub struct CapabilityOverlay {
    pub version: u64,
    pub effective_tools: Vec<String>,
    pub effective_skills: Vec<String>,
    pub effective_mcp_servers: Vec<String>,
}

impl CapabilityOverlay {
    pub fn new(tools: Vec<String>, skills: Vec<String>, mcp_servers: Vec<String>) -> Self {
        Self {
            version: 1,
            effective_tools: tools,
            effective_skills: skills,
            effective_mcp_servers: mcp_servers,
        }
    }

    /// Update overlay and bump version.
    pub fn update(&mut self, tools: Vec<String>, skills: Vec<String>, mcp_servers: Vec<String>) {
        self.effective_tools = tools;
        self.effective_skills = skills;
        self.effective_mcp_servers = mcp_servers;
        self.version += 1;
    }

    /// Check if the overlay matches the current state (no-op update).
    pub fn matches(&self, tools: &[String], skills: &[String], mcp_servers: &[String]) -> bool {
        self.effective_tools == tools
            && self.effective_skills == skills
            && self.effective_mcp_servers == mcp_servers
    }
}
