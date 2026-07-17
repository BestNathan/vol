use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug, Clone)]
pub struct CommandRecord {
    pub command_id: String,
    pub node_id: String,
    pub operation_kind: String,
    pub status: String,
    pub run_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RunRecord {
    pub run_id: String,
    pub command_id: Option<String>,
    pub node_id: String,
    pub agent_id: String,
    pub status: String,
}

pub struct CommandStore {
    records: RwLock<HashMap<String, CommandRecord>>,
}

impl CommandStore {
    pub fn new() -> Self {
        Self {
            records: RwLock::new(HashMap::new()),
        }
    }

    #[allow(clippy::expect_used)]
    pub fn insert(&self, record: CommandRecord) {
        self.records
            .write()
            .expect("command store records lock poisoned while inserting command")
            .insert(record.command_id.clone(), record);
    }

    #[allow(clippy::expect_used)]
    pub fn get(&self, command_id: &str) -> Option<CommandRecord> {
        self.records
            .read()
            .expect("command store records lock poisoned while getting command")
            .get(command_id)
            .cloned()
    }
}

impl Default for CommandStore {
    fn default() -> Self {
        Self::new()
    }
}

pub struct RunStore {
    records: RwLock<HashMap<String, RunRecord>>,
}

impl RunStore {
    pub fn new() -> Self {
        Self {
            records: RwLock::new(HashMap::new()),
        }
    }

    #[allow(clippy::expect_used)]
    pub fn insert(&self, record: RunRecord) {
        self.records
            .write()
            .expect("run store records lock poisoned while inserting run")
            .insert(record.run_id.clone(), record);
    }

    #[allow(clippy::expect_used)]
    pub fn get(&self, run_id: &str) -> Option<RunRecord> {
        self.records
            .read()
            .expect("run store records lock poisoned while getting run")
            .get(run_id)
            .cloned()
    }
}

impl Default for RunStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_store_insert_and_get() {
        let store = CommandStore::new();
        let record = CommandRecord {
            command_id: "cmd-1".to_string(),
            node_id: "node-a".to_string(),
            operation_kind: "agent.submit".to_string(),
            status: "pending".to_string(),
            run_id: None,
        };
        store.insert(record);
        let fetched = store.get("cmd-1").unwrap();
        assert_eq!(fetched.command_id, "cmd-1");
        assert_eq!(fetched.status, "pending");
    }

    #[test]
    fn command_store_get_returns_none_for_missing() {
        let store = CommandStore::new();
        assert!(store.get("no-such-command").is_none());
    }

    #[test]
    fn run_store_insert_and_get() {
        let store = RunStore::new();
        let record = RunRecord {
            run_id: "run-1".to_string(),
            command_id: Some("cmd-1".to_string()),
            node_id: "node-a".to_string(),
            agent_id: "coding".to_string(),
            status: "running".to_string(),
        };
        store.insert(record);
        let fetched = store.get("run-1").unwrap();
        assert_eq!(fetched.run_id, "run-1");
        assert_eq!(fetched.node_id, "node-a");
        assert_eq!(fetched.status, "running");
    }

    #[test]
    fn run_store_get_returns_none_for_missing() {
        let store = RunStore::new();
        assert!(store.get("no-such-run").is_none());
    }

    #[test]
    fn run_store_overwrite_same_run_id() {
        let store = RunStore::new();
        store.insert(RunRecord {
            run_id: "run-1".to_string(),
            command_id: None,
            node_id: "node-a".to_string(),
            agent_id: "coding".to_string(),
            status: "running".to_string(),
        });
        store.insert(RunRecord {
            run_id: "run-1".to_string(),
            command_id: None,
            node_id: "node-a".to_string(),
            agent_id: "coding".to_string(),
            status: "completed".to_string(),
        });
        let fetched = store.get("run-1").unwrap();
        assert_eq!(fetched.status, "completed");
    }
}
