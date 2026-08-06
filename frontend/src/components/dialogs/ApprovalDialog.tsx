// frontend/src/components/dialogs/ApprovalDialog.tsx
// HITL tool-approval modal. Reads approvalAtom, which event-handlers populate
// on the approval_request event (reqId = the run_id the event was published
// under). Gap fix over the Dioxus reference (approval_dialog.rs), whose
// Approve/Reject buttons only cleared the dialog: here they call
// agent.approve with the pending run_id and the chosen verdict, then clear
// the atom. The modal is deliberately not dismissible (no backdrop click /
// Esc / close button) — a pending request must be answered.
import { useAtom } from 'jotai'
import { Dialog, DialogContent, DialogTitle } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
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
    <Dialog
      open={pending}
      onOpenChange={(open) => {
        if (!open) {
          // no-op — HITL must be explicitly answered: no backdrop click, Esc,
          // or close button may dismiss a pending request.
        }
      }}
    >
      <DialogContent
        className="z-[100] rounded-lg w-[95vw] max-w-[600px] sm:min-w-[400px] sm:w-[90vw] sm:max-w-[500px] max-h-[80vh] overflow-y-auto"
        overlayClassName="bg-black/60"
        hideCloseButton
        onPointerDownOutside={(e) => e.preventDefault()}
        onEscapeKeyDown={(e) => e.preventDefault()}
      >
        <DialogTitle className="text-[16px] font-bold text-foreground mb-3 border-b border-border pb-2">
          Tool Approval Required
        </DialogTitle>
        <div className="text-yellow-400 font-bold text-[15px]">[!] {approval.toolName ?? 'unknown tool'}</div>
        {approval.reason !== null && approval.reason !== '' && (
          <div className="text-foreground/80 my-1.5">Reason: {approval.reason}</div>
        )}
        {approval.arguments !== null && approval.arguments !== '' && (
          <div className="font-mono text-[12px] text-muted-foreground bg-background px-2 py-1.5 rounded-md my-2 max-h-[100px] overflow-y-auto whitespace-pre-wrap">
            {approval.arguments}
          </div>
        )}
        <div className="mt-3 flex gap-2 pt-2 border-t border-border">
          <Button variant="success" size="sm" className="cursor-pointer" onClick={() => void resolve(true)}>Approve</Button>
          <Button variant="destructive" size="sm" className="cursor-pointer" onClick={() => void resolve(false)}>Reject</Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
