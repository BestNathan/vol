import { apiClient } from './client';
import type { AgentTypesResponse, AgentTypeMeta } from '../types';

export async function fetchAgentTypes(): Promise<AgentTypeMeta[]> {
  const { data } = await apiClient.get<AgentTypesResponse>('/api/v1/agent-types');
  return data.agent_types;
}
