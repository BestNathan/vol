// frontend/tests/unit/skills-dialogs.test.ts
// Unit tests for the pure helpers of Task 3.4: the Skills panel's scope badge
// color, the skill detail dialog's file-path join, and the HITL approval
// dialog's agent.approve param builder (the GAP FIX: Approve/Reject actually
// call the RPC with the pending run_id and verdict).
import { describe, expect, it } from 'vitest'
import { scopeColor } from '@/components/panels/SkillsPanel'
import { skillFilePath } from '@/components/dialogs/SkillDetailDialog'
import { buildApproveParams } from '@/components/dialogs/ApprovalDialog'

describe('scopeColor', () => {
  it('maps User to green and Repo to blue', () => {
    expect(scopeColor('User')).toBe('#40c040')
    expect(scopeColor('Repo')).toBe('#4080ff')
  })

  it('maps any other scope to gold', () => {
    expect(scopeColor('Team')).toBe('#c0c040')
    expect(scopeColor('')).toBe('#c0c040')
  })
})

describe('skillFilePath', () => {
  it('joins directory and file with a slash', () => {
    expect(skillFilePath('skills/web', 'SKILL.md')).toBe('skills/web/SKILL.md')
    expect(skillFilePath('skills/web', 'scripts/install.sh')).toBe('skills/web/scripts/install.sh')
  })

  it('returns the bare file for an empty directory', () => {
    expect(skillFilePath('', 'SKILL.md')).toBe('SKILL.md')
  })
})

describe('buildApproveParams', () => {
  it('builds agent.approve params with the pending run_id and verdict', () => {
    expect(buildApproveParams('run-42', true)).toEqual({ run_id: 'run-42', approved: true })
    expect(buildApproveParams('run-42', false)).toEqual({ run_id: 'run-42', approved: false })
  })

  it('returns null when no run_id is known', () => {
    expect(buildApproveParams(null, true)).toBeNull()
    expect(buildApproveParams('', false)).toBeNull()
  })
})
