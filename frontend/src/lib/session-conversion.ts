// frontend/src/lib/session-conversion.ts
// session_entries_to_conversation — port of
// crates/vol-llm-ui/src/web/components/sessions_panel.rs::session_entries_to_conversation.
// Raw session entries (vol-session SessionEntry, externally-tagged `data`)
// are converted into the same ConversationEntry timeline used by live runs,
// so resumed/replayed sessions render as the same entries.
//
// Wire shape (verified against the file-backed session store):
//   data.message                 → { message: SessionMessage }   (SessionEntryData::Message)
//   data.message.message         → SessionMessage wrapper (id/session_id/message/...)
//   data.message.message.message → vol_llm_core::Message (role/content/thinking/tool_calls)
//   data.checkpoint              → { reason, note }              (SessionEntryData::Checkpoint)
//   data.summary                 → { summary }                   (SessionEntryData::Summary)
// Multipart message content ([{type:"text",...},{type:"image",image_url:{url}}])
// splits into text and images; user messages render image parts as `images`.
import type { ConversationEntry } from '@/types'
import type { SessionEntry } from '@/lib/protocol'
import { formatToolArgs, truncatePreview } from '@/lib/event-handlers'

/** The vol_llm_core::Message fields we read off the wire. */
interface SessionMsg {
  role?: string
  name?: string
  content?: unknown
  thinking?: unknown
  tool_calls?: unknown
}

/** Shape of SessionEntryData (polymorphic, keyed by entry type). */
type SessionEntryDataShape = {
  message?: { message?: { message?: SessionMsg } }
  checkpoint?: { reason?: string; note?: string | null }
}

/** Split wire content into display text and image data URLs. */
function extractParts(content: unknown): { text: string; images: string[] } {
  if (typeof content === 'string') return { text: content, images: [] }
  if (Array.isArray(content)) {
    const text: string[] = []
    const images: string[] = []
    for (const part of content) {
      if (part && typeof part === 'object') {
        const rec = part as Record<string, unknown>
        if (typeof rec.text === 'string') {
          text.push(rec.text)
        } else if (rec.type === 'image') {
          const img = (rec.image_url ?? {}) as Record<string, unknown>
          if (typeof img.url === 'string') images.push(img.url)
        } else if (typeof rec.type === 'string') {
          text.push(rec.type) // unknown future part types degrade to their name
        }
      }
    }
    return { text: text.filter(Boolean).join('\n'), images }
  }
  return { text: '', images: [] }
}

export function sessionEntriesToConversation(entries: SessionEntry[]): ConversationEntry[] {
  const out: ConversationEntry[] = []
  for (const e of entries) {
    const data = (e.data ?? {}) as SessionEntryDataShape
    switch (e.type) {
      case 'message': {
        const msg = data.message?.message?.message
        if (!msg) break
        const role = msg.role ?? ''
        if (role === 'user') {
          const { text, images } = extractParts(msg.content)
          out.push({
            type: 'UserInput',
            text,
            images: images.length > 0 ? images : undefined,
          })
        } else if (role === 'assistant') {
          // Extract thinking if present, so resumed sessions show thinking blocks.
          const thinking = typeof msg.thinking === 'string' ? msg.thinking : ''
          if (thinking) out.push({ type: 'Thinking', content: thinking })
          // Extract tool_calls if present.
          if (Array.isArray(msg.tool_calls)) {
            for (const tc of msg.tool_calls) {
              const t = tc as { name?: string; arguments?: unknown }
              const fullArguments =
                typeof t.arguments === 'string' ? t.arguments : JSON.stringify(t.arguments ?? {})
              out.push({
                type: 'ToolCall',
                toolName: t.name ?? 'tool',
                argPreview: formatToolArgs(fullArguments),
                fullArguments,
              })
            }
          }
          out.push({ type: 'AgentAnswer', text: extractParts(msg.content).text })
        } else if (role === 'tool') {
          const text = extractParts(msg.content).text
          out.push({
            type: 'ToolResult',
            toolName: msg.name ?? 'tool',
            preview: truncatePreview(text, 200),
            fullResult: text,
            success: true,
          })
        }
        break
      }
      case 'checkpoint': {
        const cp = data.checkpoint
        out.push({
          type: 'EntryCheckpoint',
          reason: cp?.reason ?? 'Checkpoint',
          note: cp?.note ?? null,
          createdAt: e.created_at,
        })
        break
      }
      case 'summary': {
        out.push({ type: 'RunSummary', iterations: 0, toolCalls: 0, elapsedMs: 0 })
        break
      }
      default:
        break
    }
  }
  return out
}
