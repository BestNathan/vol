// frontend/src/stores/workspace.ts
// Workspace tab state: the lazily loaded file tree (workspaceTreeAtom), the
// open file tabs (openFilesAtom), the selected tab (selectedFileTabAtom), and
// the set of collapsed directory paths (collapsedDirsAtom).
// The per-node cache itself lives in stores/cache.ts (nodeDataCacheAtom, keys
// "files" / "workspace_tree") — FileTree reads/writes it around these atoms so
// switching nodes hydrates the tree instantly.
import { atom } from 'jotai'
import type { WorkspaceTreeNode, OpenFileTab } from '@/types'

export const workspaceTreeAtom = atom<WorkspaceTreeNode>({
  name: '',
  path: '.',
  is_dir: true,
  loaded: false,
  load_error: false,
  children: [],
})

export const openFilesAtom = atom<OpenFileTab[]>([])
export const selectedFileTabAtom = atom<number | null>(null)
export const collapsedDirsAtom = atom<Set<string>>(new Set<string>())

// --- Pure tree helpers (unit-tested) ---

/** Deep-clone a tree node. */
function cloneNode(n: WorkspaceTreeNode): WorkspaceTreeNode {
  return { ...n, children: n.children.map(cloneNode) }
}

/** Find the node at `path` (depth-first). */
export function findTreeNode(root: WorkspaceTreeNode, path: string): WorkspaceTreeNode | undefined {
  if (root.path === path) return root
  for (const child of root.children) {
    const found = findTreeNode(child, path)
    if (found) return found
  }
  return undefined
}

/** Build a child path under `dirPath` (the root is "." or ""). */
export function childPath(dirPath: string, name: string): string {
  return dirPath === '.' || dirPath === '' ? name : `${dirPath}/${name}`
}

/** Return a new tree with the children of the dir at `dirPath` replaced by
 *  `entries` and the dir marked loaded/load_error. Missing dirs leave the
 *  tree untouched. Calling with `[]` and `loaded=false` clears the dir's
 *  children (used to invalidate before a refresh). */
export function replaceDirChildren(
  root: WorkspaceTreeNode,
  dirPath: string,
  entries: readonly { name: string; is_dir: boolean }[],
  loaded = true,
  loadError = false,
): WorkspaceTreeNode {
  const copy = cloneNode(root)
  const node = findTreeNode(copy, dirPath)
  if (!node) return copy
  node.children = entries.map((e) => ({
    name: e.name,
    path: childPath(dirPath, e.name),
    is_dir: e.is_dir,
    loaded: false,
    load_error: false,
    children: [],
  }))
  node.loaded = loaded
  node.load_error = loadError
  return copy
}
