// frontend/tests/unit/task-dep-graph.test.ts
// Unit tests for the pure build_graph_layout function, ported from the Rust
// task_dep_graph.rs test module.
import { describe, it, expect } from 'vitest'
import { build_graph_layout } from '@/components/dialogs/TaskDepGraph'
import type { GraphLayout } from '@/components/dialogs/TaskDepGraph'
import type { TaskEntry } from '@/types'

function t(id: string, deps: string[], blocks: string[]): TaskEntry {
  return {
    id,
    status: 'pending',
    kind: 'task',
    publisher: null,
    assignee: null,
    subject: `task ${id}`,
    description: '',
    active_form: null,
    dependencies: deps,
    blocks,
    created_at: 0,
    started_at: null,
    completed_at: null,
  }
}

function node(layout: GraphLayout, id: string) {
  const n = layout.nodes.find((n) => n.id === id)
  expect(n, `node ${id} present`).toBeDefined()
  return n!
}

function hasEdge(layout: GraphLayout, from: string, to: string): boolean {
  return layout.edges.some((e) => e.from === from && e.to === to)
}

describe('build_graph_layout', () => {
  it('places a linear chain above and below the center', () => {
    const tasks = [t('1', [], ['2']), t('2', ['1'], ['3']), t('3', ['2'], [])]
    const layout = build_graph_layout(tasks, '2')
    expect(node(layout, '1').layer).toBe(-1)
    expect(node(layout, '2').layer).toBe(0)
    expect(node(layout, '3').layer).toBe(1)
    expect(hasEdge(layout, '1', '2')).toBe(true)
    expect(hasEdge(layout, '2', '3')).toBe(true)
    expect(layout.nodes.length).toBe(3)
  })

  it('converges a diamond at the deepest layer', () => {
    const tasks = [
      t('1', [], ['2', '3']),
      t('2', ['1'], ['4']),
      t('3', ['1'], ['4']),
      t('4', ['2', '3'], []),
    ]
    const layout = build_graph_layout(tasks, '1')
    expect(node(layout, '1').layer).toBe(0)
    expect(node(layout, '2').layer).toBe(1)
    expect(node(layout, '3').layer).toBe(1)
    expect(node(layout, '4').layer).toBe(2)
    expect(hasEdge(layout, '1', '2')).toBe(true)
    expect(hasEdge(layout, '1', '3')).toBe(true)
    expect(hasEdge(layout, '2', '4')).toBe(true)
    expect(hasEdge(layout, '3', '4')).toBe(true)
    expect(node(layout, '2').order).not.toBe(node(layout, '3').order)
  })

  it('uses the longest-path layer for asymmetric paths', () => {
    // 1 blocks 2 and 4 directly; 2 -> 3 -> 4 is a longer path to 4.
    const tasks = [
      t('1', [], ['2', '4']),
      t('2', ['1'], ['3']),
      t('3', ['2'], ['4']),
      t('4', ['1', '3'], []),
    ]
    const layout = build_graph_layout(tasks, '1')
    expect(node(layout, '1').layer).toBe(0)
    expect(node(layout, '2').layer).toBe(1)
    expect(node(layout, '3').layer).toBe(2)
    // 4 is reachable at depth 1 (1->4) and depth 3 (1->2->3->4); longest-path
    // layering must place it at the deeper layer so 3->4 points downward.
    expect(node(layout, '4').layer).toBe(3)
    expect(hasEdge(layout, '1', '4')).toBe(true)
    expect(hasEdge(layout, '3', '4')).toBe(true)
  })

  it('terminates on a cycle and places each node once', () => {
    const tasks = [t('1', ['2'], ['2']), t('2', ['1'], ['1'])]
    const layout = build_graph_layout(tasks, '1')
    expect(layout.nodes.length).toBe(2)
    expect(node(layout, '1').layer).toBe(0)
    expect(node(layout, '2').layer).toBe(-1)
  })

  it('marks an unknown referenced id as not known', () => {
    const tasks = [t('1', ['99'], [])]
    const layout = build_graph_layout(tasks, '1')
    expect(node(layout, '1').known).toBe(true)
    expect(node(layout, '99').known).toBe(false)
    expect(node(layout, '99').layer).toBe(-1)
    expect(hasEdge(layout, '99', '1')).toBe(true)
  })

  it('lays out an isolated task as a single node with no edges', () => {
    const tasks = [t('1', [], [])]
    const layout = build_graph_layout(tasks, '1')
    expect(layout.nodes.length).toBe(1)
    expect(node(layout, '1').layer).toBe(0)
    expect(layout.edges.length).toBe(0)
  })
})
