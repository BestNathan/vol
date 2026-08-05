// frontend/src/components/panels/FileContentView.tsx
// Workspace tab body: open-file tab strip (icon + name + × close with
// selection fixup) above the selected file's content in a <pre>, an error
// block, or a loading spinner. Rendered by TabContent when
// activeTabAtom === 'workspace'. Port of file_content.rs.
import { useAtom } from 'jotai'
import { openFilesAtom, selectedFileTabAtom } from '@/stores/workspace'
import { fileIcon } from '@/components/panels/FileTree'
import { cn } from '@/lib/utils'

/** Selection after closing the tab at `closedPos` (prevCount = tabs before
 *  the close): closing the selected tab picks the next one (last picks the
 *  new last), closing an earlier tab shifts the selection down. */
export function closeTabSelectionFixup(
  selected: number | null,
  closedPos: number,
  prevCount: number,
): number | null {
  if (selected === null) return null
  if (selected === closedPos) return Math.min(closedPos, Math.max(prevCount - 2, 0))
  if (selected > closedPos) return selected - 1
  return selected
}

export function FileContentView() {
  const [openFiles, setOpenFiles] = useAtom(openFilesAtom)
  const [selected, setSelected] = useAtom(selectedFileTabAtom)

  if (openFiles.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-muted-foreground/70 text-[13px]">
        Click a file in the explorer to open it
      </div>
    )
  }

  const closeTab = (pos: number) => {
    const prevCount = openFiles.length
    setOpenFiles((prev) => prev.filter((_, i) => i !== pos))
    setSelected((cur) => closeTabSelectionFixup(cur, pos, prevCount))
  }

  const selectedTab = selected !== null ? openFiles[selected] : undefined

  return (
    <div className="flex-1 flex flex-col overflow-hidden min-h-0">
      {/* Tab strip */}
      <div className="flex bg-[#1e1e38] border-b border-[#2a2a44] flex-shrink-0 overflow-x-auto">
        {openFiles.map((tab, i) => {
          const name = tab.path.split('/').pop() || tab.path
          const isSelected = selected === i
          return (
            <div
              key={tab.path}
              title={tab.path}
              className={cn(
                'px-2 py-1 text-[12px] flex items-center gap-1 cursor-pointer border-b-2 whitespace-nowrap select-none',
                isSelected
                  ? 'text-foreground bg-background border-primary'
                  : 'text-[#777] border-transparent hover:text-[#bbb] hover:bg-secondary/50',
              )}
              onClick={() => setSelected(i)}
            >
              <span className="text-[13px]">{fileIcon(name, false)}</span>
              <span className="max-w-[150px] overflow-hidden text-ellipsis">{name}</span>
              <button
                type="button"
                aria-label={`Close ${name}`}
                className="text-[10px] text-muted-foreground/60 px-0.5 rounded-[2px] leading-none hover:text-destructive hover:bg-red-950/30 cursor-pointer"
                onClick={(e) => { e.stopPropagation(); closeTab(i) }}
              >
                ✕
              </button>
            </div>
          )
        })}
      </div>

      {/* Content: error > loading > <pre> (matches file_content.rs match) */}
      {selectedTab === undefined ? null : (
        selectedTab.content === undefined && selectedTab.error !== undefined ? (
          <div className="p-3 text-destructive font-bold">Error: {selectedTab.error}</div>
        ) : selectedTab.content === undefined ? (
          <div className="flex-1 flex items-center justify-center gap-2 text-muted-foreground text-[14px]">
            <span className="w-4 h-4 rounded-full border-2 border-border border-t-[#80a0ff] animate-spin" />
            Loading...
          </div>
        ) : (
          <pre className="flex-1 overflow-auto p-3 font-mono text-[12px] leading-[1.6] text-[#c8c8e0] bg-background whitespace-pre m-0">
            {selectedTab.content}
          </pre>
        )
      )}
    </div>
  )
}
