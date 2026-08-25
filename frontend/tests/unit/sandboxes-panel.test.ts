// frontend/tests/unit/sandboxes-panel.test.ts
// Unit tests for SandboxesPanel helper: kindBadgeClass.
import { describe, expect, it } from 'vitest'
import { kindBadgeClass } from '@/components/panels/SandboxesPanel'

describe('kindBadgeClass', () => {
  it('returns emerald for local sandboxes', () => {
    expect(kindBadgeClass('local')).toContain('emerald')
  })

  it('returns blue for ssh sandboxes', () => {
    expect(kindBadgeClass('ssh')).toContain('blue')
  })

  it('returns orange for firecracker sandboxes', () => {
    expect(kindBadgeClass('firecracker')).toContain('orange')
  })

  it('returns purple for wasm sandboxes', () => {
    expect(kindBadgeClass('wasm')).toContain('purple')
  })

  it('returns secondary for tmp sandboxes', () => {
    expect(kindBadgeClass('tmp')).toContain('secondary')
  })

  it('returns secondary for unknown kinds', () => {
    expect(kindBadgeClass('unknown')).toContain('secondary')
  })
})
