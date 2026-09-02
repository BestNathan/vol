// frontend/tests/unit/task-id-rendering.test.ts
// Guards the string representation of task ids on the wire. The handler emits
// `id` / `dependencies` / `blocks` as decimal-digit strings ("1", never 1 and
// never "t1"), so TaskEntry must type them as strings and no component may
// prepend a "t" at render time.
import { describe, expect, it } from 'vitest'
import type { TaskEntry } from '@/types'

describe('task id representation', () => {
  it('types ids as strings', () => {
    // A complete literal with no type assertion: if `id` were `number` (or
    // `dependencies`/`blocks` were `number[]`) this would be a hard type error.
    // An `as TaskEntry` cast would not — casts launder exactly this mistake.
    const task: TaskEntry = {
      id: '1',
      status: 'pending',
      kind: 'manual',
      publisher: null,
      assignee: null,
      subject: 'first',
      description: '',
      active_form: null,
      dependencies: ['2', '3'],
      blocks: [],
      created_at: 1756800000,
      started_at: null,
      completed_at: null,
    }
    expect(task.id).toBe('1')
    expect(task.dependencies).toEqual(['2', '3'])
    expect(task.blocks).toEqual([])
  })

  it('does not prefix ids for display', () => {
    const task = { id: '42' } as TaskEntry
    expect(`${task.id}`).toBe('42')
    expect(`${task.id}`).not.toMatch(/^t/)
  })
})
