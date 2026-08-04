// frontend/tests/unit/context-panel.test.ts
// Unit tests for the pure helpers of Task 4.3: the context panel's anchor
// zone badge colors and the context dialog's role colors.
import { describe, expect, it } from 'vitest'
import { anchorZoneColor } from '@/components/panels/ContextPanel'
import { roleColor } from '@/components/dialogs/ContextDialog'

describe('anchorZoneColor', () => {
  it('maps head to blue, middle to gold, and tail to green', () => {
    expect(anchorZoneColor('head')).toBe('#4080ff')
    expect(anchorZoneColor('middle')).toBe('#c0a040')
    expect(anchorZoneColor('tail')).toBe('#40c040')
  })

  it('maps any other zone to neutral grey', () => {
    expect(anchorZoneColor('')).toBe('#888')
    expect(anchorZoneColor('unknown')).toBe('#888')
  })
})

describe('roleColor', () => {
  it('maps system to grey, user to blue, assistant to white, and tool to gold', () => {
    expect(roleColor('system')).toBe('#888')
    expect(roleColor('user')).toBe('#80a0ff')
    expect(roleColor('assistant')).toBe('#e0e0e0')
    expect(roleColor('tool')).toBe('#c0a040')
  })

  it('maps any other role to neutral grey', () => {
    expect(roleColor('')).toBe('#888')
    expect(roleColor('error')).toBe('#888')
  })
})
