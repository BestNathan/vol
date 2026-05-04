import { apiClient } from './client';
import type { InstancesResponse, AgentInstanceSummary } from '../types';

export async function fetchInstances(): Promise<AgentInstanceSummary[]> {
  const { data } = await apiClient.get<InstancesResponse>('/api/v1/agent-instances');
  return data.instances;
}

export async function destroyInstance(agentType: string, sessionId: string): Promise<void> {
  await apiClient.delete(`/api/v1/agent-instances/${agentType}/${sessionId}`);
}
