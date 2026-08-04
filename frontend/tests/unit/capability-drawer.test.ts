// frontend/tests/unit/capability-drawer.test.ts
import { describe, it, expect } from 'vitest'
import { filterCapabilityItems } from '@/components/inputs/CapabilityDrawer'

describe('filterCapabilityItems', () => {
  const items = [
    { name: 'bash', description: 'Run shell commands' },
    { name: 'read_file', description: 'Read a file' },
    { name: 'web_search', description: 'Search the web' },
    { name: 'write_file', description: 'Write a file' },
  ]

  it('extracts names and marks base items', () => {
    expect(filterCapabilityItems(items, ['bash', 'read_file'], '')).toEqual([
      { name: 'bash', isBase: true },
      { name: 'read_file', isBase: true },
      { name: 'web_search', isBase: false },
      { name: 'write_file', isBase: false },
    ])
  })

  it('filters case-insensitively by substring and keeps base marking', () => {
    expect(filterCapabilityItems(items, ['read_file'], 'FILE')).toEqual([
      { name: 'read_file', isBase: true },
      { name: 'write_file', isBase: false },
    ])
  })

  it('drops items without a string name and tolerates non-object entries', () => {
    const messy = [
      { name: 'ok' },
      { name: 42 },
      { name: '' },
      null,
      'not-an-object',
      undefined,
      { description: 'no name field' },
    ]
    expect(filterCapabilityItems(messy, [], '')).toEqual([{ name: 'ok', isBase: false }])
  })

  it('returns an empty list for no matches', () => {
    expect(filterCapabilityItems(items, [], 'zzz')).toEqual([])
  })
})
