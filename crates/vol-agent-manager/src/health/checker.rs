use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tracing::{debug, info};

use crate::events::{EventBus, ManagerEvent};
use crate::state::manager::AgentStateManager;
use crate::state::models::AgentStatus;

/// Periodic health checker that scans agent heartbeats.
pub struct HealthChecker {
    state_manager: Arc<AgentStateManager>,
    check_interval: Duration,
    heartbeat_timeout: Duration,
    event_bus: Option<Arc<EventBus>>,
}

impl HealthChecker {
    pub fn new(
        state_manager: Arc<AgentStateManager>,
        check_interval: Duration,
        heartbeat_timeout: Duration,
        event_bus: Option<Arc<EventBus>>,
    ) -> Self {
        Self {
            state_manager,
            check_interval,
            heartbeat_timeout,
            event_bus,
        }
    }

    /// Run a single scan of all agents.
    pub async fn run_once(&self) {
        let agents = self.state_manager.list_all().await;
        let now = Utc::now();

        for agent in &agents {
            let elapsed = now.signed_duration_since(agent.last_heartbeat);
            let timeout = chrono::Duration::from_std(self.heartbeat_timeout).unwrap();

            if elapsed > timeout && agent.status != AgentStatus::Dead {
                info!(agent_id = %agent.agent_id, "Agent heartbeat timed out, marking as dead");
                self.state_manager
                    .update_status(&agent.agent_id, AgentStatus::Dead)
                    .await;
                if let Some(ref bus) = self.event_bus {
                    bus.emit(ManagerEvent::agent_dead(&agent.agent_id));
                }
            } else if elapsed <= timeout && agent.status == AgentStatus::Dead {
                debug!(agent_id = %agent.agent_id, "Dead agent has recent heartbeat, restoring");
                self.state_manager
                    .update_status(&agent.agent_id, AgentStatus::Connected)
                    .await;
            }
        }
    }

    /// Run the checker in a loop.
    pub async fn run_loop(self: Arc<Self>) {
        loop {
            tokio::time::sleep(self.check_interval).await;
            self.run_once().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::manager::AgentStateManager;
    use crate::state::models::{AgentState, AgentStatus, HostInfo};

    fn make_state(id: &str, last_hb: chrono::DateTime<Utc>) -> AgentState {
        AgentState {
            agent_id: id.to_string(),
            name: id.to_string(),
            r#type: "test".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![],
            host_info: HostInfo {
                hostname: "h".to_string(),
                os: "linux".to_string(),
                arch: "x86_64".to_string(),
                ip: "127.0.0.1".to_string(),
            },
            status: AgentStatus::Idle,
            connected_at: Utc::now(),
            last_heartbeat: last_hb,
        }
    }

    #[tokio::test]
    async fn test_checker_marks_stale_agents_as_dead() {
        let mgr = Arc::new(AgentStateManager::new());
        let stale_time = Utc::now() - chrono::Duration::seconds(100);
        mgr.register(make_state("stale-agent", stale_time)).await;

        let checker = HealthChecker::new(
            mgr.clone(),
            Duration::from_secs(15),
            Duration::from_secs(90),
            None,
        );
        checker.run_once().await;

        let state = mgr.get("stale-agent").await.unwrap();
        assert_eq!(state.status, AgentStatus::Dead);
    }

    #[tokio::test]
    async fn test_checker_ignores_fresh_agents() {
        let mgr = Arc::new(AgentStateManager::new());
        let fresh_time = Utc::now();
        mgr.register(make_state("fresh-agent", fresh_time)).await;

        let checker = HealthChecker::new(
            mgr.clone(),
            Duration::from_secs(15),
            Duration::from_secs(90),
            None,
        );
        checker.run_once().await;

        let state = mgr.get("fresh-agent").await.unwrap();
        assert_ne!(state.status, AgentStatus::Dead);
    }
}
