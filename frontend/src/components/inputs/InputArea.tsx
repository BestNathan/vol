// frontend/src/components/inputs/InputArea.tsx
// Text input for sending messages to the agent. Sits below the capability bar
// and above the status bar region of the conversation tab. Port of the Dioxus
// input_area.rs with the Cancel-button gap fix.
import { useCallback, useMemo, useRef, useState } from 'react'
import { useAtomValue, useSetAtom } from 'jotai'
import { getPanelClient } from '@/lib/panel-client'
import { cn } from '@/lib/utils'
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
    // Attribute the upcoming run to this agent (cleared on agent_start).
    setPendingSubmitAgent(selectedAgentId)
    setText('')
    getPanelClient()
      .call<{ run_id: string }>('agent.submit', { input, target: selectedAgentId })
      .catch((err) => {
        setPendingSubmitAgent(null)
        setText(input)
        const message = (err as { message?: string } | null)?.message ?? String(err)
        const map = new Map(conversationMap)
        const conv = map.get(selectedAgentId) ?? { entries: [], autoScroll: true }
        conv.entries.push({ type: 'Error', message: `Submit failed: ${message}` })
        map.set(selectedAgentId, conv)
        setConversationMap(map)
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
      <div className="border-t border-[#333355] p-2.5 bg-[#252540] flex-shrink-0">
        <div className="text-[#f0c040] text-[13px]">
          Tool approval pending — resolve the request before sending a new message.
        </div>
      </div>
    )
  }

  return (
    <div className="border-t border-[#333355] p-2.5 bg-[#252540] flex-shrink-0 sm:px-2 sm:py-1.5">
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={handleKeyDown}
        disabled={isRunning}
        placeholder="Type a message to the agent..."
        rows={2}
        className="w-full bg-[#1a1a2e] text-[#e0e0e0] border border-[#444466] rounded-md px-2 py-1.5 text-[16px] sm:text-[14px] font-sans resize-none min-h-[40px] max-h-[120px] outline-none focus:border-[#80a0ff] disabled:opacity-50"
      />
      <div className="mt-1 flex items-center justify-between text-[10px] sm:text-[11px] text-[#666]">
        {isRunning ? (
          <div className="flex items-center gap-2">
            <span className="text-[#f0c040]">Running... (input disabled)</span>
            <button
              type="button"
              onClick={handleCancel}
              disabled={!runId}
              className={cn(
                'text-[#f0c040] cursor-pointer',
                'hover:text-[#ff8080] hover:underline',
                'disabled:text-[#666] disabled:cursor-not-allowed disabled:hover:no-underline',
              )}
            >
              Cancel
            </button>
          </div>
        ) : (
          <span>
            <span className="text-[#80a0ff] font-bold">Enter</span> Send&nbsp;&nbsp;
            <span className="text-[#80a0ff] font-bold">Shift+Enter</span> Newline&nbsp;&nbsp;
            <span className="text-[#80a0ff] font-bold">Esc×2</span> Clear
          </span>
        )}
        <button
          type="button"
          onClick={handleNewSession}
          disabled={isRunning}
          className="text-[#555] hover:text-[#c0c040] hover:underline cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
        >
          + New Session
        </button>
      </div>
    </div>
  )
}
