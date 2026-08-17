// frontend/tests/unit/input-area.test.ts
import { describe, it, expect } from 'vitest'
import { buildInputParts, findRunIdForAgent } from '@/components/inputs/InputArea'

describe('findRunIdForAgent', () => {
  it('returns the run_id owned by the agent', () => {
    const runMap = new Map([
      ['run-1', 'agent-a'],
      ['run-2', 'agent-b'],
      ['run-3', 'agent-a'],
    ])
    expect(findRunIdForAgent(runMap, 'agent-b')).toBe('run-2')
    expect(findRunIdForAgent(runMap, 'agent-a')).toBe('run-1')
  })

  it('returns null when the agent has no run', () => {
    const runMap = new Map([['run-1', 'agent-a']])
    expect(findRunIdForAgent(runMap, 'agent-z')).toBeNull()
  })

  it('returns null for an empty map', () => {
    expect(findRunIdForAgent(new Map(), 'agent-a')).toBeNull()
  })
})

describe('buildInputParts', () => {
  it('builds text + image parts in order', () => {
    expect(buildInputParts('look', ['data:image/png;base64,AAAA'])).toEqual([
      { type: 'text', text: 'look' },
      { type: 'image_url', url: 'data:image/png;base64,AAAA' },
    ])
  })

  it('omits the text part when text is empty', () => {
    expect(buildInputParts('', ['data:image/png;base64,AAAA'])).toEqual([
      { type: 'image_url', url: 'data:image/png;base64,AAAA' },
    ])
  })

  it('returns only text parts when no images', () => {
    expect(buildInputParts('hello', [])).toEqual([{ type: 'text', text: 'hello' }])
  })
})
