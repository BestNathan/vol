use std::sync::Arc;

use config::ManagerConfig;
use events::EventBus;
use metrics::collector::MetricsCollector;
use state::manager::AgentStateManager;
use task::dispatcher::TaskDispatcher;

pub mod config;
pub mod events;
pub mod health;
pub mod instance;
pub mod metrics;
pub mod state;
pub mod task;
pub mod ws;

/// Shared state passed to axum handlers.
#[derive(Clone)]
pub struct AppRouterState {
    pub state_manager: Arc<AgentStateManager>,
    pub metrics: Arc<MetricsCollector>,
    pub event_bus: Arc<EventBus>,
    pub task_dispatcher: Arc<TaskDispatcher>,
    pub config: ManagerConfig,
    pub instance_registry: Arc<crate::instance::AgentInstanceRegistry>,
    pub agent_loader: Arc<vol_llm_agent::AgentLoader>,
}
