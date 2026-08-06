// frontend/src/components/inputs/InputArea.tsx
// Text input for sending messages to the agent. Sits below the capability bar
// and above the status bar region of the conversation tab. Port of the Dioxus
// input_area.rs with the Cancel-button gap fix.
import { useCallback, useMemo, useRef, useState } from 'react'
import { useAtomValue, useSetAtom, getDefaultStore } from 'jotai'
import { Button } from '@/components/ui/button'
import { getPanelClient } from '@/lib/panel-client'
import { selectedAgentIdAtom, agentStatusMapAtom } from '@/stores/agents'
import {
  isRunningAtom, pendingSubmitAgentAtom, runMapAtom, sessionIdAtom,
} from '@/stores/connection'
import { conversationMapAtom, activeAgentIdAtom } from '@/stores/conversation'
import { approvalPendingAtom } from '@/stores/dialogs'

// Find the run_id currently owned by an agent (runMap: run_id → agent_id).
export function findRunIdForAgent(
  runMap: ReadonlyMap<string, string>,
  agentId: string,
): string | null {
  for (const [runId, owner] of runMap) {
    if (owner === agentId) return runId
  }
  return null
}

export function InputArea() {
  const [text, setText] = useState('')
  const isRunning = useAtomValue(isRunningAtom)
  const selectedAgentId = useAtomValue(selectedAgentIdAtom)
  const activeAgentId = useAtomValue(activeAgentIdAtom)
  const approvalPending = useAtomValue(approvalPendingAtom)
  const runMap = useAtomValue(runMapAtom)
  const agentStatusMap = useAtomValue(agentStatusMapAtom)
  const setSessionId = useSetAtom(sessionIdAtom)
  const setPendingSubmitAgent = useSetAtom(pendingSubmitAgentAtom)
  const conversationMap = useAtomValue(conversationMapAtom)
  const setConversationMap = useSetAtom(conversationMapAtom)
  const lastEscAtRef = useRef(0)

  // Current run_id for the selected agent — needed for Cancel. Prefer the
  // runMap (authoritative on agent_start), fall back to the agent status map
  // (populated by checkAgentRunning when selecting a mid-run agent).
  const runId = useMemo(() => {
    if (!selectedAgentId) return null
    return (
      findRunIdForAgent(runMap, selectedAgentId)
      ?? agentStatusMap[selectedAgentId]?.runId
      ?? null
    )
  }, [runMap, agentStatusMap, selectedAgentId])

  const submit = useCallback(() => {
    const input = text.trim()
    if (!input || isRunning || !selectedAgentId) return
    // Optimistic UserInput: append immediately so the user sees their message
    // even before the backend sends agent_start (which can take 20s+ during
    // MCP warm-up). If the submit fails we replace it with an Error entry.
    const map = new Map(conversationMap)
    const conv = map.get(selectedAgentId) ?? { entries: [], autoScroll: true }
    const userEntry = { type: 'UserInput' as const, text: input }
    conv.entries.push(userEntry)
    map.set(selectedAgentId, { entries: [...conv.entries], autoScroll: conv.autoScroll })
    setConversationMap(map)

    // Attribute the upcoming run to this agent (cleared on agent_start).
    setPendingSubmitAgent(selectedAgentId)
    setText('')
    const store = getDefaultStore()
    const sessionId = store.get(sessionIdAtom)
    getPanelClient()
      .call<{ run_id: string }>('agent.submit', {
        input: {
          parts: [{ type: 'text', text: input }],
          metadata: { session_id: sessionId },
        },
        target: selectedAgentId,
      })
      .catch((err) => {
        setPendingSubmitAgent(null)
        setText(input)
        const message = (err as { message?: string } | null)?.message ?? String(err)
        // Replace the optimistic UserInput with an Error entry
        const map2 = new Map(conversationMap)
        const conv2 = map2.get(selectedAgentId) ?? { entries: [], autoScroll: true }
        const idx = conv2.entries.findIndex(
          e => e.type === 'UserInput' && e.text === input && e === userEntry
        )
        if (idx !== -1) {
          conv2.entries[idx] = { type: 'Error', message: `Submit failed: ${message}` }
        } else {
          conv2.entries.push({ type: 'Error', message: `Submit failed: ${message}` })
        }
        map2.set(selectedAgentId, { entries: [...conv2.entries], autoScroll: conv2.autoScroll })
        setConversationMap(map2)
      })
  }, [text, isRunning, selectedAgentId, setPendingSubmitAgent, conversationMap, setConversationMap])

  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey) {
      // Plain Enter submits; Ctrl+Enter / Shift+Enter fall through to the
      // default textarea newline.
      e.preventDefault()
      submit()
    } else if (e.key === 'Escape') {
      // Esc twice within 500ms clears the input.
      const now = Date.now()
      if (now - lastEscAtRef.current < 500) {
        setText('')
        lastEscAtRef.current = 0
      } else {
        lastEscAtRef.current = now
      }
    }
  }, [submit])

  const handleCancel = useCallback(() => {
    if (!runId) return
    getPanelClient()
      .call('agent.cancel', { run_id: runId })
      .catch((err) => console.error('Cancel failed:', err))
  }, [runId])

  const handleNewSession = useCallback(() => {
    const newId = `web-${Date.now().toString(36)}`
    setSessionId(newId)
    const agentId = selectedAgentId ?? activeAgentId
    if (agentId) {
      const map = new Map(conversationMap)
      map.set(agentId, { entries: [], autoScroll: true })
      setConversationMap(map)
    }
  }, [selectedAgentId, activeAgentId, setSessionId, conversationMap, setConversationMap])

  // While a tool approval is pending, the textarea is replaced by a banner so
  // the user cannot send a new message before resolving the dialog.
  if (approvalPending) {
    return (
      <div className="border-t border-border p-2.5 bg-card flex-shrink-0">
        <div className="text-yellow-400 text-[13px]">
          Tool approval pending — resolve the request before sending a new message.
        </div>
      </div>
    )
  }

  return (
    <div className="border-t border-border p-2.5 bg-card flex-shrink-0 sm:px-2 sm:py-1.5">
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={handleKeyDown}
        disabled={isRunning}
        placeholder="Type a message to the agent..."
        rows={2}
        className="w-full bg-background text-foreground border border-input rounded-md px-2 py-1.5 text-[16px] sm:text-[14px] font-sans resize-none min-h-[40px] max-h-[120px] outline-none focus:border-primary disabled:opacity-50"
      />
      <div className="mt-1 flex items-center justify-between text-[10px] sm:text-[11px] text-muted-foreground/70">
        {isRunning ? (
          <div className="flex items-center gap-2">
            <span className="text-yellow-400">Running... (input disabled)</span>
            <Button
              variant="ghost"
              size="sm"
              className="cursor-pointer text-yellow-400 hover:text-destructive/80 text-[10px] sm:text-[11px]"
              onClick={handleCancel}
              disabled={!runId}
            >
              Cancel
            </Button>
          </div>
        ) : (
          <span>
            <span className="text-primary font-bold">Enter</span> Send&nbsp;&nbsp;
            <span className="text-primary font-bold">Shift+Enter</span> Newline&nbsp;&nbsp;
            <span className="text-primary font-bold">Esc×2</span> Clear
          </span>
        )}
        <Button
          variant="ghost"
          size="sm"
          className="cursor-pointer text-muted-foreground/60 hover:text-yellow-400/70 text-[10px] sm:text-[11px]"
          onClick={handleNewSession}
          disabled={isRunning}
        >
          + New Session
        </Button>
      </div>
    </div>
  )
}
