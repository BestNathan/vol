import { describe, expect, it } from 'vitest'
import type { RpcMethods } from '@/lib/protocol'

describe('agent.submit params', () => {
  it('accepts the structured input shape actually sent', () => {
    const params: RpcMethods['agent.submit']['params'] = {
      input: {
        parts: [{ type: 'text', text: 'hi' }],
        metadata: { session_id: 's1' },
        task_ids: ['1', '2'],
      },
      target: 'agent-a',
    }
    expect(params.input.task_ids).toEqual(['1', '2'])
  })

  it('allows task_ids to be omitted', () => {
    const params: RpcMethods['agent.submit']['params'] = {
      input: { parts: [{ type: 'text', text: 'hi' }] },
    }
    expect(params.input.task_ids).toBeUndefined()
  })
})
