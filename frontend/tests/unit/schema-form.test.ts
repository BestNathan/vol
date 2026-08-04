// frontend/tests/unit/schema-form.test.ts
import { describe, it, expect } from 'vitest'
import { collectMissingDefaults } from '@/components/inputs/SchemaForm'

describe('collectMissingDefaults', () => {
  it('pre-populates explicit defaults for unset keys', () => {
    const schema = {
      type: 'object',
      properties: {
        command: { type: 'string', description: 'Shell command' },
        timeout: { type: 'integer', default: 30 },
        retries: { type: 'integer', default: 3 },
      },
    }
    expect(collectMissingDefaults(schema, {})).toEqual({ timeout: 30, retries: 3 })
  })

  it('leaves already-set keys untouched', () => {
    const schema = {
      type: 'object',
      properties: {
        timeout: { type: 'integer', default: 30 },
        command: { type: 'string', default: 'ls' },
      },
    }
    expect(collectMissingDefaults(schema, { timeout: 99 })).toEqual({ command: 'ls' })
  })

  it('collects a nested object default when the property carries an explicit default', () => {
    const schema = {
      type: 'object',
      properties: {
        model: {
          type: 'object',
          default: { temperature: 0.7 },
          properties: { temperature: { type: 'number', default: 0.7 } },
        },
      },
    }
    expect(collectMissingDefaults(schema, {})).toEqual({ model: { temperature: 0.7 } })
  })

  it('does not materialize nested objects without an explicit default', () => {
    const schema = {
      type: 'object',
      properties: {
        model: {
          type: 'object',
          properties: { temperature: { type: 'number', default: 0.7 } },
        },
      },
    }
    expect(collectMissingDefaults(schema, {})).toEqual({})
  })

  it('treats non-undefined values (even null) as set', () => {
    const schema = {
      type: 'object',
      properties: {
        flag: { type: 'boolean', default: true },
      },
    }
    expect(collectMissingDefaults(schema, { flag: null })).toEqual({})
  })

  it('returns {} when the schema has no properties', () => {
    expect(collectMissingDefaults({ type: 'object' }, {})).toEqual({})
    expect(collectMissingDefaults({}, {})).toEqual({})
    expect(collectMissingDefaults({ properties: 'not-an-object' }, {})).toEqual({})
  })

  it('does not invent defaults for keys without a default field', () => {
    const schema = {
      type: 'object',
      properties: {
        command: { type: 'string' },
        enabled: { type: 'boolean' },
      },
    }
    expect(collectMissingDefaults(schema, {})).toEqual({})
  })
})
