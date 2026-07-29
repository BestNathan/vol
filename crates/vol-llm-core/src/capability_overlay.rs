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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_new_has_version_one() {
        let overlay = CapabilityOverlay::new(
            vec!["bash".into(), "read".into()],
            vec!["code-review".into()],
            vec!["k8s".into()],
        );
        assert_eq!(overlay.version, 1);
        assert_eq!(overlay.effective_tools.len(), 2);
        assert_eq!(overlay.effective_skills.len(), 1);
        assert_eq!(overlay.effective_mcp_servers.len(), 1);
    }

    #[test]
    fn overlay_update_bumps_version() {
        let mut overlay = CapabilityOverlay::new(vec!["bash".into()], vec![], vec![]);
        assert_eq!(overlay.version, 1);
        overlay.update(vec!["bash".into(), "write".into()], vec![], vec![]);
        assert_eq!(overlay.version, 2);
        assert_eq!(overlay.effective_tools.len(), 2);
    }

    #[test]
    fn overlay_matches_detects_no_change() {
        let overlay = CapabilityOverlay::new(vec!["bash".into(), "read".into()], vec![], vec![]);
        assert!(overlay.matches(&["bash".into(), "read".into()], &[], &[]));
        assert!(!overlay.matches(&["bash".into()], &[], &[]));
        assert!(!overlay.matches(&["bash".into(), "read".into(), "write".into()], &[], &[]));
    }

    #[test]
    fn overlay_version_persists_across_updates() {
        let mut overlay = CapabilityOverlay::new(vec![], vec![], vec![]);
        overlay.update(vec!["a".into()], vec![], vec![]);
        overlay.update(vec!["a".into(), "b".into()], vec![], vec![]);
        overlay.update(vec!["c".into()], vec![], vec![]);
        assert_eq!(overlay.version, 4);
    }

    #[test]
    fn overlay_empty_lists_are_allowed() {
        let overlay = CapabilityOverlay::new(vec![], vec![], vec![]);
        assert_eq!(overlay.effective_tools.len(), 0);
        assert!(overlay.matches(&[], &[], &[]));
    }
}
