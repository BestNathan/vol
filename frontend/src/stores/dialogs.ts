// frontend/src/stores/dialogs.ts
// Dialog state atoms. Task 2.5 only needs the approval-pending flag (the
// InputArea banner); Task 3.5 extends this store with the full approvalAtom
// (toolName/reason/arguments/reqId) and the MCP/skill/dialog atoms.
import { atom } from 'jotai'

// True while a tool approval request is pending (set on the approval_request
// event, cleared on approval_resolved). Consumed by InputArea to show the
// "Tool approval pending" banner in place of the textarea.
export const approvalPendingAtom = atom(false)
