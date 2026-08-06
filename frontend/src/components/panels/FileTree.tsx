// frontend/src/components/panels/FileTree.tsx
// Left sidebar file explorer: lazily loaded directory tree via file.list
// (per-node cached under "files" / "workspace_tree"), click-to-open file tabs
// via file.read, per-directory ⟳ refresh, and a mobile drawer (vertical rail
// + overlay) gated by fileTreeDrawerOpenAtom. Port of file_tree.rs.
import { useCallback, useEffect, useRef, useState } from 'react'
import { useAtom, useAtomValue, useSetAtom, useStore } from 'jotai'
import { getPanelClient } from '@/lib/panel-client'
import { getCacheKey, nodeDataCacheAtom } from '@/stores/cache'
import { activeNodeIdAtom, activeTabAtom, fileTreeDrawerOpenAtom } from '@/stores/ui'
import {
  workspaceTreeAtom, openFilesAtom, selectedFileTabAtom, collapsedDirsAtom,
  replaceDirChildren,
} from '@/stores/workspace'
import { cn } from '@/lib/utils'
import type { RpcMethods } from '@/lib/protocol'
import type { FileEntry, WorkspaceTreeNode } from '@/types'

/** Per-node cache keys for the workspace tree (mirrors file_tree.rs). */
export const FILES_CACHE_KEY = 'files'
export const WORKSPACE_TREE_CACHE_KEY = 'workspace_tree'

/** Root node of a fresh (unloaded) tree. */
export const ROOT_NODE: WorkspaceTreeNode = {
  name: '', path: '.', is_dir: true, loaded: false, load_error: false, children: [],
}

function errMsg(err: unknown): string {
  return (err as { message?: string } | null)?.message ?? String(err)
}

/** Emoji icon for a file extension or directory (mirrors file_tree.rs::file_icon). */
export function fileIcon(name: string, isDir: boolean): string {
  if (isDir) return '📂'
  const dot = name.lastIndexOf('.')
  const ext = dot >= 0 ? name.slice(dot + 1).toLowerCase() : ''
  switch (ext) {
    case 'rs': return '🦀'
    case 'toml':
    case 'lock': return '⚙️'
    case 'md': return '📝'
    case 'json': return '📊'
    case 'yaml':
    case 'yml':
    case 'js':
    case 'ts': return '📜'
    case 'sh':
    case 'bash': return '🐚'
    case 'html':
    case 'htm': return '🌐'
    case 'css': return '🎨'
    default: return '📄'
  }
}

/** True when `raw` looks like a cached WorkspaceTreeNode (cache guard). */
export function isWorkspaceTreeNode(raw: unknown): raw is WorkspaceTreeNode {
  const n = raw as WorkspaceTreeNode | null
  return typeof n === 'object' && n !== null
    && typeof n.name === 'string'
    && typeof n.path === 'string'
    && typeof n.is_dir === 'boolean'
    && Array.isArray(n.children)
}

/** Build a loaded root tree from a file.list result. */
export function buildRootFromEntries(entries: readonly FileEntry[]): WorkspaceTreeNode {
  return {
    ...ROOT_NODE,
    loaded: true,
    children: entries.map((e) => ({
      name: e.name,
      path: e.name,
      is_dir: e.is_dir,
      loaded: false,
      load_error: false,
      children: [],
    })),
  }
}

/** Outer panel classes: mobile rail (40px) or drawer when open, 240px sidebar
 *  from `sm:` up. */
export function fileTreeOuterClass(drawerOpen: boolean): string {
  if (drawerOpen) {
    return 'absolute inset-y-0 left-0 z-50 flex w-[80vw] max-w-[300px] flex-col overflow-hidden border-r border-[#2a2a44] bg-[#16162a] sm:static sm:z-auto sm:w-[240px] sm:flex-shrink-0'
  }
  return 'flex h-full min-h-0 w-10 flex-shrink-0 flex-col overflow-hidden border-r border-[#2a2a44] bg-[#16162a] sm:w-[240px]'
}

/** Panel content classes: visible in the drawer, or only on desktop. */
export function fileTreeContentClass(drawerOpen: boolean): string {
  return drawerOpen
    ? 'flex min-h-0 flex-1 flex-col'
    : 'hidden min-h-0 flex-1 flex-col sm:flex'
}

/** One row in the tree: a directory (chevron + lazy load + ⟳ refresh) or a
 *  file (click opens a tab). */
function TreeNode({ node, depth }: { node: WorkspaceTreeNode; depth: number }) {
  const setTree = useSetAtom(workspaceTreeAtom)
  const [collapsedDirs, setCollapsedDirs] = useAtom(collapsedDirsAtom)
  const openFiles = useAtomValue(openFilesAtom)
  const setOpenFiles = useSetAtom(openFilesAtom)
  const setSelectedTab = useSetAtom(selectedFileTabAtom)
  const setActiveTab = useSetAtom(activeTabAtom)
  const nodeId = useAtomValue(activeNodeIdAtom)

  // Live node mirror for the stale-response guard in async callbacks.
  const nodeIdRef = useRef(nodeId)
  useEffect(() => { nodeIdRef.current = nodeId }, [nodeId])

  // Latest request id per path: a file.list response only applies when its id
  // still matches, so a ⟳ refresh issued after an in-flight request wins.
  const reqIdsRef = useRef<Map<string, number>>(new Map())

  const loadDir = (path: string) => {
    const targetNode = nodeIdRef.current
    const id = (reqIdsRef.current.get(path) ?? 0) + 1
    reqIdsRef.current.set(path, id)
    getPanelClient().call<RpcMethods['file.list']['result']>('file.list', { path })
      .then((res) => {
        if (nodeIdRef.current !== targetNode) return
        if (reqIdsRef.current.get(path) !== id) return
        const entries = res.entries ?? []
        setTree((tree) => replaceDirChildren(tree, path, entries))
        // Unloaded dirs are visually collapsed; once loaded, expand it.
        setCollapsedDirs((prev) => {
          const next = new Set(prev)
          next.delete(path)
          return next
        })
      })
      .catch(() => {
        if (nodeIdRef.current !== targetNode) return
        if (reqIdsRef.current.get(path) !== id) return
        setTree((tree) => replaceDirChildren(tree, path, [], true, true))
      })
  }

  if (node.is_dir) {
    const collapsed = collapsedDirs.has(node.path) || (!node.loaded && node.children.length === 0)

    const onDirClick = () => {
      if (!node.loaded) {
        // Lazy load; expand once the response lands.
        setCollapsedDirs((prev) => {
          const next = new Set(prev)
          next.delete(node.path)
          return next
        })
        loadDir(node.path)
      } else if (collapsedDirs.has(node.path)) {
        setCollapsedDirs((prev) => {
          const next = new Set(prev)
          next.delete(node.path)
          return next
        })
      } else {
        setCollapsedDirs((prev) => new Set(prev).add(node.path))
      }
    }

    const onRefresh = (e: React.MouseEvent) => {
      e.stopPropagation()
      // Clear current children to indicate refresh, then reload.
      setTree((tree) => replaceDirChildren(tree, node.path, [], false, false))
      loadDir(node.path)
    }

    return (
      <div>
        <div
          className="group flex items-center gap-1 py-0.5 pr-1 pl-0 cursor-pointer text-[13px] whitespace-nowrap select-none rounded mx-1 hover:bg-secondary active:bg-[#3a3a54]"
          style={{ paddingLeft: `${depth * 16 + 4}px` }}
          onClick={onDirClick}
        >
          <span className={cn(
            'w-3 h-3 flex-shrink-0 origin-center transition-transform duration-150',
            !collapsed && 'rotate-90',
          )}>
            <span className="block h-1.5 w-1.5 origin-center border-r-2 border-t-2 border-[#8b8baa] rotate-45" />
          </span>
          <span className="inline-flex items-center justify-center w-[18px] h-[18px] flex-shrink-0 text-[14px]">
            {fileIcon(node.name, true)}
          </span>
          <span className="min-w-0 flex-1 overflow-hidden text-ellipsis text-[#8ab4ff] font-medium">{node.name}</span>
          <span
            title="Refresh"
            className="ml-auto inline-flex h-5 w-5 flex-shrink-0 items-center justify-center rounded text-[12px] text-[#777799] opacity-0 transition-all duration-150 hover:bg-[#33334f] hover:text-foreground group-hover:opacity-100"
            onClick={onRefresh}
          >
            ⟳
          </span>
        </div>
        {!collapsed && (
          <div className="overflow-hidden">
            {node.children.map((child) => (
              <TreeNode key={child.path} node={child} depth={depth + 1} />
            ))}
          </div>
        )}
      </div>
    )
  }

  // File row: click opens (or selects) a tab, reads content async, and
  // switches the active tab to Workspace.
  const onFileClick = () => {
    const path = node.path
    const existing = openFiles.findIndex((t) => t.path === path)
    if (existing >= 0) {
      setSelectedTab(existing)
    } else {
      const next = [...openFiles, { path }]
      setOpenFiles(next)
      setSelectedTab(next.length - 1)
      getPanelClient().call<RpcMethods['file.read']['result']>('file.read', { path })
        .then((res) => {
          setOpenFiles((prev) => prev.map((t) => (t.path === path ? { ...t, content: res.content } : t)))
        })
        .catch((err) => {
          const msg = errMsg(err)
          setOpenFiles((prev) => prev.map((t) => (t.path === path ? { ...t, error: msg } : t)))
        })
    }
    setActiveTab('workspace')
  }

  return (
    <div
      className="flex items-center gap-1 py-0.5 pr-2 pl-0 cursor-pointer text-[13px] whitespace-nowrap select-none rounded mx-1 hover:bg-secondary active:bg-[#3a3a54]"
      style={{ paddingLeft: `${depth * 16 + 4}px` }}
      onClick={onFileClick}
    >
      <span className="inline-flex items-center justify-center w-5 h-5 flex-shrink-0 text-[10px] text-muted-foreground/70 invisible">▾</span>
      <span className="inline-flex items-center justify-center w-[18px] h-[18px] flex-shrink-0 text-[14px]">
        {fileIcon(node.name, false)}
      </span>
      <span className="min-w-0 overflow-hidden text-ellipsis text-foreground/80">{node.name}</span>
    </div>
  )
}

/** Left sidebar file tree. */
export function FileTree() {
  const store = useStore()
  const nodeId = useAtomValue(activeNodeIdAtom)
  const [tree, setTree] = useAtom(workspaceTreeAtom)
  const setCollapsedDirs = useSetAtom(collapsedDirsAtom)
  const [drawerOpen, setDrawerOpen] = useAtom(fileTreeDrawerOpenAtom)
  const setCache = useSetAtom(nodeDataCacheAtom)
  const [rootLoading, setRootLoading] = useState(false)
  const [rootError, setRootError] = useState<string | null>(null)

  // Live node mirror for the stale-response guard in async callbacks.
  const nodeIdRef = useRef(nodeId)
  useEffect(() => { nodeIdRef.current = nodeId }, [nodeId])

  // Load the root listing for `target`: hydrate from the per-node cache
  // ("workspace_tree" full tree, or "files" root entries) when present,
  // otherwise file.list(".") and write both back to the cache. Writes are
  // dropped once the active node no longer matches the fetch target.
  const loadRoot = useCallback(async (target: string | null) => {
    if (!target) {
      setTree(ROOT_NODE)
      setRootLoading(false)
      setRootError(null)
      return
    }
    const treeCacheKey = getCacheKey(target, WORKSPACE_TREE_CACHE_KEY)
    const cachedTree = store.get(nodeDataCacheAtom).get(treeCacheKey)?.get(WORKSPACE_TREE_CACHE_KEY)
    if (isWorkspaceTreeNode(cachedTree)) {
      setTree(cachedTree)
      setRootLoading(false)
      setRootError(null)
      return
    }
    const filesCacheKey = getCacheKey(target, FILES_CACHE_KEY)
    const cachedEntries = store.get(nodeDataCacheAtom).get(filesCacheKey)?.get(FILES_CACHE_KEY)
    if (Array.isArray(cachedEntries)) {
      setTree(buildRootFromEntries(cachedEntries as FileEntry[]))
      setRootLoading(false)
      setRootError(null)
      return
    }
    setRootLoading(true)
    setRootError(null)
    try {
      const res = await getPanelClient().call<RpcMethods['file.list']['result']>('file.list', { path: '.' })
      if (nodeIdRef.current !== target) return
      const entries = res.entries ?? []
      const nextTree = buildRootFromEntries(entries)
      setTree(nextTree)
      // Write back to cache for instant switching.
      setCache((prev) => {
        const next = new Map(prev)
        next.set(filesCacheKey, new Map<string, unknown>([[FILES_CACHE_KEY, entries]]))
        next.set(treeCacheKey, new Map<string, unknown>([[WORKSPACE_TREE_CACHE_KEY, nextTree]]))
        return next
      })
    } catch (err) {
      if (nodeIdRef.current !== target) return
      setRootError(errMsg(err))
    } finally {
      if (nodeIdRef.current === target) setRootLoading(false)
    }
  }, [setTree, setCache, store])

  // Load on mount and whenever the active node changes; collapse the tree
  // (open files stay open — they are global, not per-node).
  useEffect(() => {
    setCollapsedDirs(new Set())
    void loadRoot(nodeId)
  }, [nodeId, loadRoot, setCollapsedDirs])

  // Keep the backdrop mounted for 200ms after close so the fade-out can play
  // before unmounting.
  const [backdropVisible, setBackdropVisible] = useState(false)
  useEffect(() => {
    if (drawerOpen) {
      setBackdropVisible(true)
      return
    }
    const t = setTimeout(() => setBackdropVisible(false), 200)
    return () => clearTimeout(t)
  }, [drawerOpen])

  // True while the drawer is sliding out (backdrop still fading).
  const closing = backdropVisible && !drawerOpen

  return (
    <>
      {/* Backdrop — mobile only, positioned inside the content flex row */}
      {backdropVisible && (
        <div
          className={cn(
            'sm:hidden absolute inset-0 z-40 bg-black/50',
            drawerOpen ? 'animate-in fade-in-0 duration-200' : 'animate-out fade-out-0 duration-200'
          )}
          onClick={() => setDrawerOpen(false)}
        />
      )}
      {/* Mobile rail — visible when the drawer is closed. */}
      {!drawerOpen && (
        <button
          type="button"
          aria-label="Open file explorer"
          className="sm:hidden flex h-full w-10 flex-shrink-0 cursor-pointer flex-col items-center gap-2 border-0 border-r border-[#2a2a44] bg-[#16162a] px-0 py-3 text-[#8b8baa] hover:bg-secondary hover:text-foreground"
          onClick={() => setDrawerOpen(true)}
        >
          <span className="text-[16px] leading-none">📂</span>
          <span className="text-[10px] font-semibold uppercase" style={{ writingMode: 'vertical-rl' }}>
            Files
          </span>
        </button>
      )}
      {/* Drawer panel — slides in from the left on mobile; static sidebar on
          desktop. Always mounted so the transform can transition both ways. */}
      <div
        className={cn(
          fileTreeOuterClass(true),
          'transition-transform duration-200',
          drawerOpen ? 'translate-x-0' : '-translate-x-full sm:translate-x-0'
        )}
      >
        <div className={cn(fileTreeContentClass(drawerOpen), closing && 'flex')}>
          <div className="px-3 py-2 text-[11px] font-semibold uppercase tracking-[0.8px] text-[#6a6a9a] border-b border-[#2a2a44] flex-shrink-0 flex items-center justify-between">
            <span>Explorer</span>
            <button
              type="button"
              aria-label="Close file explorer"
              className="sm:hidden text-muted-foreground hover:text-foreground text-[16px] cursor-pointer"
              onClick={() => setDrawerOpen(false)}
            >
              ✕
            </button>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto py-1">
            {!nodeId ? (
              <div className="flex items-center justify-center h-full text-muted-foreground/70 p-5 text-center text-[12px]">
                No node selected
              </div>
            ) : rootError !== null && tree.children.length === 0 ? (
              <div className="flex flex-col items-center gap-3 p-4 text-center">
                <div className="text-destructive text-[12px]">Failed to load files</div>
                <div className="text-muted-foreground text-[11px] break-words">{rootError}</div>
                <button
                  type="button"
                  className="text-[11px] text-primary hover:text-[#a0c0ff] cursor-pointer"
                  onClick={() => void loadRoot(nodeId)}
                >
                  Retry
                </button>
              </div>
            ) : rootLoading && tree.children.length === 0 ? (
              <div className="flex items-center justify-center h-full text-muted-foreground/70 p-5 text-center text-[12px]">
                Loading files...
              </div>
            ) : tree.children.length === 0 ? (
              <div className="flex items-center justify-center h-full text-muted-foreground/70 p-5 text-center text-[12px]">
                (empty)
              </div>
            ) : (
              tree.children.map((child) => (
                <TreeNode key={child.path} node={child} depth={0} />
              ))
            )}
          </div>
        </div>
      </div>
    </>
  )
}
