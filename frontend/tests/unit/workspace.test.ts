// frontend/tests/unit/workspace.test.ts
import { describe, it, expect } from 'vitest'
import {
  buildRootFromEntries, fileIcon, fileTreeContentClass, fileTreeOuterClass,
  isWorkspaceTreeNode,
} from '@/components/panels/FileTree'
import { closeTabSelectionFixup } from '@/components/panels/FileContentView'
import { childPath, findTreeNode, replaceDirChildren } from '@/stores/workspace'
import type { WorkspaceTreeNode } from '@/types'

const sampleTree: WorkspaceTreeNode = {
  name: '', path: '.', is_dir: true, loaded: true, load_error: false,
  children: [
    { name: 'src', path: 'src', is_dir: true, loaded: true, load_error: false, children: [
      { name: 'main.rs', path: 'src/main.rs', is_dir: false, loaded: false, load_error: false, children: [] },
    ] },
    { name: 'Cargo.toml', path: 'Cargo.toml', is_dir: false, loaded: false, load_error: false, children: [] },
  ],
}

describe('fileIcon', () => {
  it('maps directories to the folder emoji', () => {
    expect(fileIcon('src', true)).toBe('📂')
  })

  it('maps .rs to the crab', () => {
    expect(fileIcon('main.rs', false)).toBe('🦀')
  })

  it('maps .toml and .lock to the gear', () => {
    expect(fileIcon('Cargo.toml', false)).toBe('⚙️')
    expect(fileIcon('Cargo.lock', false)).toBe('⚙️')
  })

  it('maps .md to the memo', () => {
    expect(fileIcon('README.md', false)).toBe('📝')
  })

  it('maps .json to the chart', () => {
    expect(fileIcon('data.json', false)).toBe('📊')
  })

  it('maps .yaml/.yml/.js/.ts to the scroll', () => {
    expect(fileIcon('deploy.yaml', false)).toBe('📜')
    expect(fileIcon('config.yml', false)).toBe('📜')
    expect(fileIcon('app.js', false)).toBe('📜')
    expect(fileIcon('app.ts', false)).toBe('📜')
  })

  it('maps .sh to the shell', () => {
    expect(fileIcon('run.sh', false)).toBe('🐚')
  })

  it('maps .html to the globe and .css to the palette', () => {
    expect(fileIcon('index.html', false)).toBe('🌐')
    expect(fileIcon('styles.css', false)).toBe('🎨')
  })

  it('falls back to the page for unknown or missing extensions', () => {
    expect(fileIcon('Makefile', false)).toBe('📄')
    expect(fileIcon('notes.txt', false)).toBe('📄')
  })

  it('is case-insensitive on the extension', () => {
    expect(fileIcon('MAIN.RS', false)).toBe('🦀')
  })
})

describe('workspace tree helpers', () => {
  it('finds a node by path depth-first', () => {
    expect(findTreeNode(sampleTree, 'src')?.name).toBe('src')
    expect(findTreeNode(sampleTree, 'src/main.rs')?.path).toBe('src/main.rs')
    expect(findTreeNode(sampleTree, 'nope')).toBeUndefined()
  })

  it('builds child paths relative to the root', () => {
    expect(childPath('.', 'src')).toBe('src')
    expect(childPath('', 'src')).toBe('src')
    expect(childPath('src', 'main.rs')).toBe('src/main.rs')
  })

  it('replaces the children of a nested dir and marks it loaded', () => {
    const next = replaceDirChildren(sampleTree, 'src', [
      { name: 'main.rs', is_dir: false },
      { name: 'lib', is_dir: true },
    ])
    const src = findTreeNode(next, 'src')
    expect(src?.children.map((c) => c.path)).toEqual(['src/main.rs', 'src/lib'])
    expect(src?.loaded).toBe(true)
    expect(src?.load_error).toBe(false)
    expect(src?.children[0].is_dir).toBe(false)
    expect(src?.children[1].is_dir).toBe(true)
    // untouched elsewhere
    expect(findTreeNode(next, 'Cargo.toml')).toBeDefined()
  })

  it('marks a dir unloaded and load_error on failure', () => {
    const next = replaceDirChildren(sampleTree, 'src', [], true, true)
    const src = findTreeNode(next, 'src')
    expect(src?.children).toEqual([])
    expect(src?.loaded).toBe(true)
    expect(src?.load_error).toBe(true)
  })

  it('leaves the tree untouched when the dir is missing', () => {
    const next = replaceDirChildren(sampleTree, 'missing', [{ name: 'x', is_dir: false }])
    expect(next).toEqual(sampleTree)
  })
})

describe('workspace tree cache guard', () => {
  it('accepts a full tree shape and rejects garbage', () => {
    expect(isWorkspaceTreeNode(sampleTree)).toBe(true)
    expect(isWorkspaceTreeNode(null)).toBe(false)
    expect(isWorkspaceTreeNode({ name: 'x' })).toBe(false)
    expect(isWorkspaceTreeNode([1, 2])).toBe(false)
  })

  it('builds a loaded root from file.list entries', () => {
    const root = buildRootFromEntries([
      { name: 'src', is_dir: true, size: 0 },
      { name: 'Cargo.toml', is_dir: false, size: 10 },
    ])
    expect(root.loaded).toBe(true)
    expect(root.children.map((c) => c.path)).toEqual(['src', 'Cargo.toml'])
    expect(root.children[0].is_dir).toBe(true)
  })
})

describe('file tree panel classes', () => {
  it('closed mobile tree is a 40px rail; desktop is a 240px sidebar', () => {
    const cls = fileTreeOuterClass(false)
    expect(cls).toContain('w-10')
    expect(cls).toContain('flex-shrink-0')
    expect(cls).toContain('sm:w-[240px]')
  })

  it('open drawer overlays the content area; desktop stays a static sidebar', () => {
    const cls = fileTreeOuterClass(true)
    expect(cls).toContain('absolute')
    expect(cls).toContain('z-50')
    expect(cls).toContain('sm:static')
    expect(cls).not.toContain('fixed')
  })

  it('panel content is hidden when the drawer is closed, visible on desktop', () => {
    const cls = fileTreeContentClass(false)
    expect(cls).toContain('hidden')
    expect(cls).toContain('sm:flex')
  })
})

describe('closeTabSelectionFixup', () => {
  it('closing the selected tab picks the next tab', () => {
    expect(closeTabSelectionFixup(2, 2, 5)).toBe(2)
  })

  it('closing the last tab picks the new last tab', () => {
    expect(closeTabSelectionFixup(4, 4, 5)).toBe(3)
  })

  it('closing a tab before the selection shifts it down', () => {
    expect(closeTabSelectionFixup(3, 1, 5)).toBe(2)
  })

  it('closing a tab after the selection leaves it unchanged', () => {
    expect(closeTabSelectionFixup(1, 3, 5)).toBe(1)
  })

  it('closing the only tab keeps a valid (now-empty) selection', () => {
    expect(closeTabSelectionFixup(0, 0, 1)).toBe(0)
  })

  it('keeps null selection null', () => {
    expect(closeTabSelectionFixup(null, 0, 3)).toBeNull()
  })
})
