// frontend/src/components/dialogs/ApprovalDialog.tsx
// HITL tool-approval modal. Reads approvalAtom, which event-handlers populate
// on the approval_request event (reqId = the run_id the event was published
// under). Gap fix over the Dioxus reference (approval_dialog.rs), whose
// Approve/Reject buttons only cleared the dialog: here they call
// agent.approve with the pending run_id and the chosen verdict, then clear
// the atom. The modal is deliberately not dismissible (no backdrop click /
// Esc / close button) — a pending request must be answered.
import { useAtom } from 'jotai'
import { approvalAtom } from '@/stores/dialogs'
import { getPanelClient } from '@/lib/panel-client'
import type { RpcMethods } from '@/lib/protocol'

/**
 * Params for the agent.approve call given the pending run_id and verdict.
 * Returns null when no run_id is known (the call cannot be issued).
 */
export function buildApproveParams(
  reqId: string | null,
  approved: boolean,
): RpcMethods['agent.approve']['params'] | null {
  if (!reqId || reqId === '') return null
  return { run_id: reqId, approved }
}

function errMsg(err: unknown): string {
  return (err as { message?: string } | null)?.message ?? String(err)
}

export function ApprovalDialog() {
  const [approval, setApproval] = useAtom(approvalAtom)
  const pending = approval.toolName !== null || approval.reqId !== null
  if (!pending) return null

  const clear = () => setApproval({ toolName: null, reason: null, arguments: null, reqId: null })

  const resolve = async (approved: boolean) => {
    const params = buildApproveParams(approval.reqId, approved)
    if (!params) {
      // No run_id to answer — nothing to tell the agent, so just drop the
      // dialog rather than leaving the banner up forever.
      console.error('ApprovalDialog: no run_id for pending approval; cleared without RPC')
      clear()
      return
    }
    try {
      await getPanelClient().call<RpcMethods['agent.approve']['result']>('agent.approve', params)
    } catch (err) {
      console.error('agent.approve failed:', errMsg(err))
    } finally {
      clear()
    }
  }

  return (
    <div
      className="fixed inset-0 bg-black/60 flex items-center justify-center z-[100]"
      role="dialog"
      aria-modal="true"
      aria-label="Tool approval required"
    >
      <div className="bg-[#252540] border border-[#444466] rounded-lg p-3 sm:p-4 w-[95vw] max-w-[600px] sm:min-w-[400px] sm:w-[90vw] sm:max-w-[500px] max-h-[80vh] overflow-y-auto">
        <div className="text-[16px] font-bold text-[#e0e0e0] mb-3 border-b border-[#333355] pb-2">
          Tool Approval Required
        </div>
        <div className="text-[#f0c040] font-bold text-[15px]">[!] {approval.toolName ?? 'unknown tool'}</div>
        {approval.reason !== null && approval.reason !== '' && (
          <div className="text-[#ccc] my-1.5">Reason: {approval.reason}</div>
        )}
        {approval.arguments !== null && approval.arguments !== '' && (
          <div className="font-mono text-[12px] text-[#888] bg-[#1a1a2e] px-2 py-1.5 rounded-md my-2 max-h-[100px] overflow-y-auto whitespace-pre-wrap">
            {approval.arguments}
          </div>
        )}
        <div className="mt-3 flex gap-2 pt-2 border-t border-[#333355]">
          <button
            type="button"
            onClick={() => void resolve(true)}
            className="px-3 py-1.5 border-none rounded-md cursor-pointer text-[13px] bg-[#408040] text-[#e0e0e0] hover:bg-[#50a050]"
          >
            Approve
          </button>
          <button
            type="button"
            onClick={() => void resolve(false)}
            className="px-3 py-1.5 border-none rounded-md cursor-pointer text-[13px] bg-[#804040] text-[#e0e0e0] hover:bg-[#905050]"
          >
            Reject
          </button>
        </div>
      </div>
    </div>
  )
}
