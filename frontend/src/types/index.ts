export interface AgentTypeMeta {
  name: string;
  type: string;
  description: string;
  scope: string;
}

export interface AgentInstanceSummary {
  agent_type: string;
  session_id: string;
  parent_session_id: string | null;
  status: string;
  connection_count: number;
  created_at: string;
}

export interface WsConnected {
  message_type: 'connected';
  agent_type: string;
  session_id: string;
}

export interface WsAgentComplete {
  message_type: 'agent_complete';
  content: string;
  iterations: number;
}

export interface WsAgentError {
  message_type: 'agent_error';
  error: string;
}

export type WsMessage = WsConnected | WsAgentComplete | WsAgentError;

export interface ManagerEvent {
  event_type: string;
  timestamp: string;
  payload: Record<string, unknown>;
}

export interface HealthResponse {
  status: string;
}

export interface AgentTypesResponse {
  agent_types: AgentTypeMeta[];
}

export interface InstancesResponse {
  instances: AgentInstanceSummary[];
}
