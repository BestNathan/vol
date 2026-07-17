//! TOML config schema for a CLI tool.
use serde::Deserialize;
use vol_llm_sandbox::registry::SandboxConfig;

#[derive(Debug, Clone, Deserialize)]
pub struct CliToolConfig {
    pub name: String,
    pub description: String,
    pub binaries: Vec<String>,

    /// Whether this tool is enabled. Default: true.
    /// Set to false for example configs that should not be loaded.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Inline sandbox config (XOR with `sandbox_ref`).
    #[serde(default)]
    pub sandbox: Option<SandboxConfig>,

    /// Name of a sandbox in `.agents/sandboxes/` (XOR with `sandbox`).
    #[serde(default)]
    pub sandbox_ref: Option<String>,

    /// Remote working directory. Supports `{{env.VAR}}`.
    pub cwd: String,

    /// Shell used to wrap the command. Default: `/bin/sh`.
    #[serde(default = "default_shell")]
    pub shell: String,

    /// Command timeout in seconds. Default: 60.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,

    /// Per-stream truncation ceiling in bytes. Default: 65536.
    #[serde(default = "default_max_output_bytes")]
    pub max_output_bytes: usize,

    /// Environment variables passed to the remote process.
    /// Values support `{{env.VAR}}` placeholders.
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

fn default_shell() -> String {
    "/bin/sh".to_string()
}
fn default_timeout_secs() -> u64 {
    60
}
fn default_max_output_bytes() -> usize {
    65_536
}
fn default_enabled() -> bool {
    true
}

impl CliToolConfig {
    /// Parse a TOML document into a CliToolConfig.
    /// Returns an error if required fields are missing, or if both
    /// `sandbox` and `sandbox_ref` are set.
    pub fn from_toml(s: &str) -> Result<Self, crate::CliToolError> {
        let cfg: CliToolConfig =
            toml::from_str(s).map_err(|e| crate::CliToolError::Config(e.to_string()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    fn validate(&self) -> Result<(), crate::CliToolError> {
        if self.binaries.is_empty() {
            return Err(crate::CliToolError::Config(
                "`binaries` must be non-empty".into(),
            ));
        }
        match (&self.sandbox, &self.sandbox_ref) {
            (Some(_), Some(_)) => Err(crate::CliToolError::Config(
                "set either `sandbox` or `sandbox_ref`, not both".into(),
            )),
            (None, None) => Err(crate::CliToolError::Config(
                "one of `sandbox` or `sandbox_ref` is required".into(),
            )),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_REF: &str = r#"
        name = "ansible"
        description = "Ansible suite"
        binaries = ["ansible", "ansible-playbook"]
        sandbox_ref = "ansible-prod"
        cwd = "/opt/ansible"
    "#;

    const FULL_INLINE: &str = r#"
        name = "ansible"
        description = "Ansible suite"
        binaries = ["ansible", "ansible-playbook"]
        cwd = "/opt/ansible"
        shell = "/bin/bash"
        timeout_secs = 300
        max_output_bytes = 131072

        [sandbox]
        name = "ansible-sandbox"
        type = "ssh"
        work_dir = "/"

        [sandbox.ssh]
        host = "ansible-prod.example.com"
        port = 22
        user = "deploy"
        identity_file = "/home/u/.ssh/id_ed25519"

        [env]
        ANSIBLE_CONFIG = "/opt/ansible/ansible.cfg"
    "#;

    #[test]
    fn parse_minimal_with_sandbox_ref() {
        let cfg = CliToolConfig::from_toml(MINIMAL_REF).unwrap();
        assert_eq!(cfg.name, "ansible");
        assert_eq!(cfg.binaries, vec!["ansible", "ansible-playbook"]);
        assert_eq!(cfg.sandbox_ref.as_deref(), Some("ansible-prod"));
        assert!(cfg.sandbox.is_none());
        assert_eq!(cfg.shell, "/bin/sh"); // default
        assert_eq!(cfg.timeout_secs, 60); // default
        assert_eq!(cfg.max_output_bytes, 65536); // default
        assert!(cfg.env.is_empty());
    }

    #[test]
    fn parse_full_with_inline_sandbox() {
        let cfg = CliToolConfig::from_toml(FULL_INLINE).unwrap();
        assert_eq!(cfg.shell, "/bin/bash");
        assert_eq!(cfg.timeout_secs, 300);
        assert_eq!(cfg.max_output_bytes, 131072);
        assert!(cfg.sandbox.is_some());
        assert!(cfg.sandbox_ref.is_none());
        let sb = cfg.sandbox.as_ref().unwrap();
        assert_eq!(sb.sandbox_type, "ssh");
        assert_eq!(sb.ssh.as_ref().unwrap().host, "ansible-prod.example.com");
        assert_eq!(
            cfg.env.get("ANSIBLE_CONFIG").map(String::as_str),
            Some("/opt/ansible/ansible.cfg")
        );
    }

    #[test]
    fn reject_both_sandbox_and_ref() {
        let bad = r#"
            name = "x"
            description = "x"
            binaries = ["x"]
            cwd = "/"
            sandbox_ref = "foo"
            [sandbox]
            name = "bar"
            type = "local"
        "#;
        let err = CliToolConfig::from_toml(bad).unwrap_err().to_string();
        assert!(err.contains("not both"), "unexpected error: {err}");
    }

    #[test]
    fn reject_neither_sandbox_nor_ref() {
        let bad = r#"
            name = "x"
            description = "x"
            binaries = ["x"]
            cwd = "/"
        "#;
        let err = CliToolConfig::from_toml(bad).unwrap_err().to_string();
        assert!(err.contains("required"), "unexpected error: {err}");
    }

    #[test]
    fn reject_empty_binaries() {
        let bad = r#"
            name = "x"
            description = "x"
            binaries = []
            sandbox_ref = "foo"
            cwd = "/"
        "#;
        let err = CliToolConfig::from_toml(bad).unwrap_err().to_string();
        assert!(err.contains("non-empty"), "unexpected error: {err}");
    }

    #[test]
    fn reject_missing_required_field() {
        let bad = r#"
            name = "x"
            binaries = ["x"]
            sandbox_ref = "foo"
            cwd = "/"
        "#; // missing description
        let err = CliToolConfig::from_toml(bad).unwrap_err().to_string();
        assert!(!err.is_empty());
    }
}
