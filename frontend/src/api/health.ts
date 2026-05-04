import { apiClient } from './client';
import type { HealthResponse } from '../types';

export async function checkHealth(): Promise<boolean> {
  try {
    const { data } = await apiClient.get<HealthResponse>('/health');
    return data.status === 'ok';
  } catch {
    return false;
  }
}
