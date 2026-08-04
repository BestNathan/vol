// frontend/src/stores/cache.ts
import { atom } from 'jotai'

// Per-node JSON cache, keyed by [nodeId, cacheKey]
// e.g., cacheMap.get('{"nodeId":"n1","key":"tools"}') = serialized tools data
export const nodeDataCacheAtom = atom<Map<string, Map<string, unknown>>>(new Map())

export function getCacheKey(nodeId: string, key: string): string {
  return JSON.stringify({ nodeId, key })
}
