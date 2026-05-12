# Design Spec: Tailwind CSS Migration for vol-llm-ui Web Frontend

## Context
- **Requirements:** `docs/superpowers/requirement/2026-05-12-tailwind-css-migration-requirement.md`
- **Current state:** All CSS in `app.rs` `GLOBAL_CSS` const (~215 lines, ~100 classes). No external CSS files.
- **Dioxus version:** 0.7 (tailwind-rs-dioxus is incompatible — requires Dioxus 0.3)
- **Node.js:** v24 available

## Architecture

### Build Pipeline

```
src/web/input.css ──┐
                     ├──> @tailwindcss/cli ──> dist/tailwind.css ──┐
src/web/components/ ─┘                                              │
                                                                     ├──> basic-http-server / browser
crates/vol-llm-ui/ ──> cargo build ──> wasm-bindgen ──> dist/wasm/ ─┘
```

### Decision: Direct Tailwind Utility Classes

Every CSS class in rsx! `class:` attributes is replaced with Tailwind utility class strings. No `@apply` layer, no intermediate class names. Dynamic state-based classes use full if/else string literals so Tailwind's scanner finds every variant.

## Components

### input.css (`src/web/input.css` — NEW)

```css
@import "tailwindcss";

@source "./components/*.rs";

@theme {
  --breakpoint-sm: 480px;
  --breakpoint-md: 768px;
  --breakpoint-lg: 1024px;
}
```

`@source` tells Tailwind v4 which Rust files contain class strings. The `glob:` prefix pattern scans all component files.

### index.html (`src/web/index.html`)

Add `<link rel="stylesheet" href="tailwind.css">` to `<head>`. Remove the Trunk `<link data-trunk rel="rust" ...>` line — WASM is loaded via JS module from the `dist/` directory built by `rebuild-web.sh`.

### rebuild-web.sh (`scripts/rebuild-web.sh`)

New step 1: Run `npx @tailwindcss/cli -i src/web/input.css -o dist/tailwind.css` before the WASM build.

### app.rs (`src/web/components/app.rs`)

- **Remove:** `GLOBAL_CSS` const and `<style { {GLOBAL_CSS} }>` from rsx!
- **Replace:** All `class: "..."` strings with equivalent Tailwind utilities

### All other components (15 files)

Replace `class: "..."` strings with Tailwind utilities. Dynamic class construction (`if active { "tab active" } else { "tab" }`) becomes full string alternatives for each branch so Tailwind's scanner discovers every class.

## Data Flow

1. User runs `scripts/rebuild-web.sh`
2. Tailwind scans all `.rs` component files, extracts class strings from `class: "..."` patterns
3. Tailwind generates `dist/tailwind.css` containing only referenced utilities
4. Cargo builds WASM binary, wasm-bindgen processes it
5. `index.html` + `tailwind.css` + WASM files copied to `dist/`
6. `basic-http-server` serves `dist/` on `0.0.0.0:8080`

## CSS Migration Reference

Below is the complete mapping of every existing CSS class to its Tailwind equivalent.

### Base / Reset

| Current CSS | Tailwind Equivalent |
|---|---|
| `* { margin:0; padding:0; box-sizing:border-box; }` | (Tailwind default) |
| `body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; font-size: 14px; color: #e0e0e0; background: #1a1a2e; }` | (on root element) `font-[system-ui] text-[14px] text-[#e0e0e0] bg-[#1a1a2e]` |

### App Container & Layout

| Class | Current CSS | Tailwind Equivalent |
|---|---|---|
| `.app-container` | `display:flex; flex-direction:column; height:100dvh; width:100vw; overflow:hidden;` | `flex flex-col h-[100dvh] w-[100vw] overflow-hidden` |
| `.main-layout` | `display:flex; flex:1; overflow:hidden;` | `flex flex-1 overflow-hidden` |
| `.sidebar` | `width:240px; min-width:180px; border-right:1px solid #2a2a44; display:flex; flex-direction:column; overflow:hidden; flex-shrink:0; background:#16162a;` | `w-[240px] min-w-[180px] border-r border-[#2a2a44] flex flex-col overflow-hidden flex-shrink-0 bg-[#16162a]` |
| `.right-panel` | `flex:1; display:flex; flex-direction:column; overflow:hidden;` | `flex-1 flex flex-col overflow-hidden` |

### Status Bar

| Class | Current CSS | Tailwind Equivalent |
|---|---|---|
| `.status-bar` | `display:flex; align-items:center; justify-content:space-between; padding:4px 12px; background:#2d2d44; color:#e0e0e0; font-size:12px; font-family:monospace; flex-shrink:0;` | `flex items-center justify-between px-3 py-1 bg-[#2d2d44] text-[#e0e0e0] text-[12px] font-mono flex-shrink-0` |
| `.status-left` | `display:flex; align-items:center; gap:6px; overflow:hidden; flex-wrap:nowrap;` | `flex items-center gap-1.5 overflow-hidden flex-nowrap` |
| `.status-right` | `display:flex; align-items:center; flex-shrink:0;` | `flex items-center flex-shrink-0` |
| `.status-item` | `white-space:nowrap;` | `whitespace-nowrap` |
| `.status-divider` | `color:#555; user-select:none;` | `text-[#555] select-none` |
| `.status-badge` | `padding:1px 6px; border-radius:3px; font-size:11px; font-weight:bold;` | `px-1.5 py-0.5 rounded-[3px] text-[11px] font-bold` |
| `.badge-running` | `background:#3a3a20; color:#f0c040;` | `bg-[#3a3a20] text-[#f0c040]` |
| `.badge-idle` | `background:#203a20; color:#80c080;` | `bg-[#203a20] text-[#80c080]` |
| `.badge-unsafe` | `background:#3a2020; color:#ff4040;` | `bg-[#3a2020] text-[#ff4040]` |
| `.badge-exiting` | `background:#3a2020; color:#ff8080;` | `bg-[#3a2020] text-[#ff8080]` |
| `.conn-indicator` | `display:flex; align-items:center; gap:4px; margin-right:4px;` | `flex items-center gap-1 mr-1` |
| `.conn-dot` | `width:8px; height:8px; border-radius:50%; display:inline-block; flex-shrink:0;` | `w-2 h-2 rounded-full inline-block flex-shrink-0` |
| `.conn-dot-connected` | `box-shadow:0 0 4px #40c040;` | `shadow-[0_0_4px_#40c040]` |
| `.conn-dot-connecting` | `animation:conn-pulse 1.5s ease-in-out infinite;` | `animate-pulse` (or custom `@keyframes`) |
| `.conn-dot-error` | `animation:conn-blink 1s ease-in-out infinite;` | Custom animation needed |
| `.conn-label` | `font-size:10px; color:#888; max-width:80px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;` | `text-[10px] text-[#888] max-w-[80px] overflow-hidden text-ellipsis whitespace-nowrap` |
| `.build-info` | `display:flex; align-items:center; font-size:11px; color:#888; flex-shrink:0;` | `flex items-center text-[11px] text-[#888] flex-shrink-0` |
| `.build-label` | `color:#666;` | `text-[#666]` |
| `.build-version` | `color:#a0a0c0; font-weight:bold;` | `text-[#a0a0c0] font-bold` |
| `.build-separator` | `color:#555; margin:0 2px;` | `text-[#555] mx-0.5` |
| `.build-time` | `color:#666;` | `text-[#666]` |

### Tab Bar

| Class | Current CSS | Tailwind Equivalent |
|---|---|---|
| `.tab-bar` | `display:flex; background:#252540; border-bottom:1px solid #333355; flex-shrink:0;` | `flex bg-[#252540] border-b border-[#333355] flex-shrink-0` |
| `.tab` | `padding:6px 16px; background:transparent; border:none; color:#888; cursor:pointer; font-size:13px; border-bottom:2px solid transparent;` + hover | `px-4 py-1.5 bg-transparent border-none text-[#888] cursor-pointer text-[13px] border-b-2 border-transparent hover:text-[#ccc] hover:bg-[#2a2a44]` |
| `.tab.active` | `color:#e0e0e0; background:#1a1a2e; border-bottom:2px solid #80a0ff;` | `text-[#e0e0e0] bg-[#1a1a2e] border-b-2 border-[#80a0ff]` |

### Conversation

| Class | Current CSS | Tailwind Equivalent |
|---|---|---|
| `.conversation` | `flex:1; overflow-y:auto; padding:10px;` | `flex-1 overflow-y-auto p-2.5` |
| `.conversation-empty` | `display:flex; align-items:center; justify-content:center; height:100%; color:#666;` | `flex items-center justify-center h-full text-[#666]` |
| `.msg` | `margin-bottom:10px; padding:8px 10px; border-radius:6px; max-width:100%; word-wrap:break-word; white-space:pre-wrap;` | `mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap` |
| `.msg-user` | `background:#1a2a44; border-left:3px solid #4080ff;` | `bg-[#1a2a44] border-l-[3px] border-[#4080ff]` |
| `.msg-user-prefix` | `color:#4080ff; font-weight:bold;` | `text-[#4080ff] font-bold` |
| `.msg-thinking` | `background:#2a2a20; border-left:3px solid #c0c040;` | `bg-[#2a2a20] border-l-[3px] border-[#c0c040]` |
| `.msg-thinking-prefix` | `color:#c0c040; font-weight:bold;` | `text-[#c0c040] font-bold` |
| `.msg-thinking-content` | `color:#888; margin-top:4px; padding-left:4px;` | `text-[#888] mt-1 pl-1` |
| `.msg-streaming` | `color:#ccc;` | `text-[#ccc]` |
| `.msg-tool` | `background:#1a2a3a; border-left:3px solid #4080c0;` | `bg-[#1a2a3a] border-l-[3px] border-[#4080c0]` |
| `.msg-tool-name` | `color:#4080c0; font-weight:bold;` | `text-[#4080c0] font-bold` |
| `.msg-tool-arg` | `color:#888; font-size:12px; margin-top:2px; padding-left:4px;` | `text-[#888] text-[12px] mt-0.5 pl-1` |
| `.msg-tool-result` | `background:#1a2a1a; border-left:3px solid #40c040;` | `bg-[#1a2a1a] border-l-[3px] border-[#40c040]` |
| `.msg-tool-result-error` | `background:#2a1a1a; border-left:3px solid #c04040;` | `bg-[#2a1a1a] border-l-[3px] border-[#c04040]` |
| `.msg-tool-result-prefix` | `font-weight:bold;` | `font-bold` |
| `.msg-tool-result-content` | `color:#888; font-size:12px; margin-top:4px; padding-left:4px; max-height:120px; overflow-y:auto; font-family:monospace;` | `text-[#888] text-[12px] mt-1 pl-1 max-h-[120px] overflow-y-auto font-mono` |
| `.msg-answer` | `color:#e0e0e0; line-height:1.5;` | `text-[#e0e0e0] leading-[1.5]` |
| `.msg-summary` | `color:#80c080; font-weight:bold; padding:6px 0;` | `text-[#80c080] font-bold py-1.5` |
| `.msg-error` | `color:#ff6060; font-weight:bold; background:#2a1a1a; border-left:3px solid #c04040;` | `text-[#ff6060] font-bold bg-[#2a1a1a] border-l-[3px] border-[#c04040]` |
| `.msg-checkpoint` | `background:#2a2a20; border-left:3px solid #c0a040; color:#aaa; font-size:12px; font-style:italic; padding:6px 10px;` | `bg-[#2a2a20] border-l-[3px] border-[#c0a040] text-[#aaa] text-[12px] italic px-2.5 py-1.5` |

### Input Area

| Class | Current CSS | Tailwind Equivalent |
|---|---|---|
| `.input-area` | `border-top:1px solid #333355; padding:8px 10px; background:#252540; flex-shrink:0;` | `border-t border-[#333355] p-2.5 bg-[#252540] flex-shrink-0` |
| `.input-row` | `display:flex; gap:8px;` | `flex gap-2` |
| `.input-area textarea` | `flex:1; background:#1a1a2e; color:#e0e0e0; border:1px solid #444466; border-radius:4px; padding:6px 8px; font-size:14px; font-family:inherit; resize:none; min-height:40px; max-height:120px; outline:none;` + focus + disabled | `flex-1 bg-[#1a1a2e] text-[#e0e0e0] border border-[#444466] rounded-md px-2 py-1.5 text-[14px] font-sans resize-none min-h-[40px] max-h-[120px] outline-none focus:border-[#80a0ff] disabled:opacity-50` |
| `.input-area button` | `padding:6px 16px; background:#4060c0; color:#e0e0e0; border:none; border-radius:4px; cursor:pointer; font-size:14px; align-self:flex-end;` + hover + disabled | `px-4 py-1.5 bg-[#4060c0] text-[#e0e0e0] border-none rounded-md cursor-pointer text-[14px] self-end hover:bg-[#5070d0] disabled:bg-[#333355] disabled:cursor-not-allowed` |
| `.input-hint` | `margin-top:4px; font-size:11px; color:#666;` | `mt-1 text-[11px] text-[#666]` |
| `.input-hint-key` | `color:#80a0ff; font-weight:bold;` | `text-[#80a0ff] font-bold` |
| `.input-hint-running` | `color:#f0c040;` | `text-[#f0c040]` |

### Workspace

| Class | Current CSS | Tailwind Equivalent |
|---|---|---|
| `.workspace-panel` | `flex:1; overflow-y:auto; padding:10px;` | `flex-1 overflow-y-auto p-2.5` |
| `.workspace-empty` | `display:flex; align-items:center; justify-content:center; height:100%; color:#666;` | `flex items-center justify-center h-full text-[#666]` |
| `.workspace-entry` | `padding:2px 0; font-family:monospace; font-size:13px;` | `py-0.5 font-mono text-[13px]` |
| `.workspace-dir` | `color:#6090ff; font-weight:bold;` | `text-[#6090ff] font-bold` |
| `.workspace-file` | `color:#e0e0e0;` | `text-[#e0e0e0]` |

### Skills / Logs

| Class | Current CSS | Tailwind Equivalent |
|---|---|---|
| `.skills-panel` | `flex:1; overflow-y:auto; padding:10px;` | `flex-1 overflow-y-auto p-2.5` |
| `.skills-empty` | `display:flex; align-items:center; justify-content:center; height:100%; color:#666;` | `flex items-center justify-center h-full text-[#666]` |
| `.skills-table` | `width:100%; border-collapse:collapse;` | `w-full border-collapse` |
| `.skills-table th` | `text-align:left; padding:4px 8px; border-bottom:1px solid #333355; font-size:12px; color:#888;` | `text-left px-2 py-1 border-b border-[#333355] text-[12px] text-[#888]` |
| `.skills-table td` | `padding:4px 8px; font-size:13px; border-bottom:1px solid #2a2a44;` | `px-2 py-1 text-[13px] border-b border-[#2a2a44]` |
| `.log-viewer` | `flex:1; overflow-y:auto; padding:10px;` | `flex-1 overflow-y-auto p-2.5` |
| `.log-run-list` | `font-family:monospace; font-size:13px;` | `font-mono text-[13px]` |
| `.log-run-item` | `padding:3px 0; color:#ccc;` | `py-0.5 text-[#ccc]` |
| `.log-run-item-id` | `color:#c0c0c0;` | `text-[#c0c0c0]` |
| `.log-run-item-count` | `color:#888;` | `text-[#888]` |
| `.log-entry` | `font-family:monospace; font-size:12px; padding:2px 0; white-space:nowrap;` | `font-mono text-[12px] py-0.5 whitespace-nowrap` |
| `.log-entry-time` | `color:#666;` | `text-[#666]` |
| `.log-entry-type` | `font-weight:bold;` | `font-bold` |
| `.log-empty` | `display:flex; align-items:center; justify-content:center; height:100%; color:#666;` | `flex items-center justify-center h-full text-[#666]` |

### Modals & Dialogs

| Class | Current CSS | Tailwind Equivalent |
|---|---|---|
| `.modal-overlay` | `position:fixed; top:0; left:0; right:0; bottom:0; background:rgba(0,0,0,0.6); display:flex; align-items:center; justify-content:center; z-index:100;` | `fixed inset-0 bg-black/60 flex items-center justify-center z-[100]` |
| `.modal-content` | `background:#252540; border:1px solid #444466; border-radius:8px; padding:16px; min-width:400px; max-width:600px; max-height:80vh; overflow-y:auto;` | `bg-[#252540] border border-[#444466] rounded-lg p-4 min-w-[400px] max-w-[600px] max-h-[80vh] overflow-y-auto` |
| `.modal-title` | `font-size:16px; font-weight:bold; color:#e0e0e0; margin-bottom:12px; border-bottom:1px solid #333355; padding-bottom:8px;` | `text-[16px] font-bold text-[#e0e0e0] mb-3 border-b border-[#333355] pb-2` |
| `.modal-empty` | `color:#888; padding:10px 0;` | `text-[#888] py-2.5` |
| `.modal-session-item` | `padding:6px 8px; border-bottom:1px solid #2a2a44; display:flex; align-items:center; gap:8px;` | `px-2 py-1.5 border-b border-[#2a2a44] flex items-center gap-2` |
| `.modal-session-item.selected` | `background:#2a2a44;` | `bg-[#2a2a44]` |
| `.modal-session-id` | `font-family:monospace; color:#e0e0e0; font-weight:bold;` | `font-mono text-[#e0e0e0] font-bold` |
| `.modal-session-meta` | `color:#888; font-size:12px;` | `text-[#888] text-[12px]` |
| `.modal-actions` | `margin-top:12px; display:flex; gap:8px; padding-top:8px; border-top:1px solid #333355;` | `mt-3 flex gap-2 pt-2 border-t border-[#333355]` |
| `.modal-actions button` | `padding:6px 12px; border:none; border-radius:4px; cursor:pointer; font-size:13px;` | `px-3 py-1.5 border-none rounded-md cursor-pointer text-[13px]` |

### Buttons

| Class | Current CSS | Tailwind Equivalent |
|---|---|---|
| `.btn-new` | `background:#4060c0; color:#e0e0e0;` | `bg-[#4060c0] text-[#e0e0e0]` |
| `.btn-resume` | `background:#408040; color:#e0e0e0;` | `bg-[#408040] text-[#e0e0e0]` |
| `.btn-delete` | `background:#804040; color:#e0e0e0;` | `bg-[#804040] text-[#e0e0e0]` |
| `.btn-cancel` | `background:#555; color:#e0e0e0;` | `bg-[#555] text-[#e0e0e0]` |
| `.btn-approve` | `background:#408040; color:#e0e0e0;` | `bg-[#408040] text-[#e0e0e0]` |
| `.btn-reject` | `background:#804040; color:#e0e0e0;` | `bg-[#804040] text-[#e0e0e0]` |
| `.btn-stop` | `background:#662020; color:#e0e0e0;` | `bg-[#662020] text-[#e0e0e0]` |

### Approval Dialog

| Class | Current CSS | Tailwind Equivalent |
|---|---|---|
| `.approval-tool-name` | `color:#f0c040; font-weight:bold; font-size:15px;` | `text-[#f0c040] font-bold text-[15px]` |
| `.approval-reason` | `color:#ccc; margin:6px 0;` | `text-[#ccc] my-1.5` |
| `.approval-args` | `font-family:monospace; font-size:12px; color:#888; background:#1a1a2e; padding:6px 8px; border-radius:4px; margin:8px 0; max-height:100px; overflow-y:auto; white-space:pre-wrap;` | `font-mono text-[12px] text-[#888] bg-[#1a1a2e] px-2 py-1.5 rounded-md my-2 max-h-[100px] overflow-y-auto whitespace-pre-wrap` |

### File Tree

| Class | Current CSS | Tailwind Equivalent |
|---|---|---|
| `.sidebar-header` | `padding:8px 12px; font-size:11px; font-weight:600; text-transform:uppercase; letter-spacing:0.8px; color:#6a6a9a; border-bottom:1px solid #2a2a44; flex-shrink:0;` | `px-3 py-2 text-[11px] font-semibold uppercase tracking-[0.8px] text-[#6a6a9a] border-b border-[#2a2a44] flex-shrink-0` |
| `.file-tree` | `flex:1; overflow-y:auto; padding:4px 0;` | `flex-1 overflow-y-auto py-1` |
| `.file-tree-empty` | `display:flex; align-items:center; justify-content:center; height:100%; color:#666; padding:20px; text-align:center; font-size:12px;` | `flex items-center justify-center h-full text-[#666] p-5 text-center text-[12px]` |
| `.file-tree-node` | `display:flex; align-items:center; padding:3px 8px 3px 0; cursor:pointer; font-size:13px; white-space:nowrap; user-select:none; border-radius:3px; margin:0 4px;` + hover | `flex items-center py-0.5 pr-2 pl-0 cursor-pointer text-[13px] whitespace-nowrap select-none rounded-[3px] mx-1 hover:bg-[#2a2a44] active:bg-[#3a3a54]` |
| `.file-tree-dir:hover` | `background:#1a2a3a;` | `hover:bg-[#1a2a3a]` |
| `.file-tree-dir .file-tree-label` | `color:#8ab4ff; font-weight:500;` | `text-[#8ab4ff] font-medium` |
| `.file-tree-file .file-tree-label` | `color:#ccc;` | `text-[#ccc]` |
| `.file-tree-chevron` | `display:inline-flex; align-items:center; justify-content:center; width:16px; height:16px; flex-shrink:0; font-size:10px; color:#666; transition:transform 0.15s;` | `inline-flex items-center justify-center w-4 h-4 flex-shrink-0 text-[10px] text-[#666] transition-transform duration-150` |
| `.file-tree-chevron.collapsed` | `transform:rotate(-90deg);` | `-rotate-90` |
| `.file-tree-chevron.hidden` | `visibility:hidden;` | `invisible` |
| `.file-tree-icon` | `display:inline-flex; align-items:center; justify-content:center; width:18px; height:18px; flex-shrink:0; margin-right:4px; font-size:14px;` | `inline-flex items-center justify-center w-[18px] h-[18px] flex-shrink-0 mr-1 text-[14px]` |
| `.file-tree-label` | `overflow:hidden; text-overflow:ellipsis;` | `overflow-hidden text-ellipsis` |
| `.file-tree-children` | `overflow:hidden;` | `overflow-hidden` |
| `.file-tree-refresh` | `font-size:10px; color:#666; margin-left:4px; opacity:0; transition:opacity 0.15s; cursor:pointer;` + parent hover | `text-[10px] text-[#666] ml-1 opacity-0 transition-opacity duration-150 cursor-pointer group-hover:opacity-100 hover:text-[#aaa]` |

### Tools Tab

| Class | Current CSS | Tailwind Equivalent |
|---|---|---|
| `.tools-tab` | `flex:1; overflow-y:auto; padding:8px;` | `flex-1 overflow-y-auto p-2` |
| `.tools-tab-empty` | `display:flex; align-items:center; justify-content:center; height:100%; color:#666;` | `flex items-center justify-center h-full text-[#666]` |
| `.tool-call-item` | `border-bottom:1px solid #2a2a44;` | `border-b border-[#2a2a44]` |
| `.tool-call-header` | `display:flex; align-items:center; padding:8px 10px; cursor:pointer; gap:8px;` + hover | `flex items-center px-2.5 py-2 cursor-pointer gap-2 hover:bg-[#222240]` |
| `.tool-call-seq` | `color:#555; font-size:11px; min-width:24px;` | `text-[#555] text-[11px] min-w-[24px]` |
| `.tool-call-name` | `font-weight:600; font-size:13px;` | `font-semibold text-[13px]` |
| `.tool-call-status` | `font-size:11px; padding:1px 6px; border-radius:3px;` | `text-[11px] px-1.5 py-0.5 rounded-[3px]` |
| `.tool-call-duration` | `font-size:11px; color:#888; margin-left:auto;` | `text-[11px] text-[#888] ml-auto` |
| `.tool-call-chevron` | `font-size:10px; color:#666; margin-left:4px;` | `text-[10px] text-[#666] ml-1` |
| `.tool-call-detail` | `padding:8px 10px 8px 42px; font-size:12px; font-family:monospace; color:#888; background:#16162a; white-space:pre-wrap; word-break:break-all;` | `px-2.5 pb-2 pl-[42px] text-[12px] font-mono text-[#888] bg-[#16162a] whitespace-pre-wrap break-all` |
| `.tool-detail-label` | `color:#6090ff; font-weight:600; font-family:sans-serif;` | `text-[#6090ff] font-semibold font-sans` |

### File Content Viewer

| Class | Current CSS | Tailwind Equivalent |
|---|---|---|
| `.file-content-view` | `flex:1; display:flex; flex-direction:column; overflow:hidden;` | `flex-1 flex flex-col overflow-hidden` |
| `.file-tab-bar` | `display:flex; background:#1e1e38; border-bottom:1px solid #2a2a44; flex-shrink:0; overflow-x:auto;` | `flex bg-[#1e1e38] border-b border-[#2a2a44] flex-shrink-0 overflow-x-auto` |
| `.file-tab` | `padding:4px 8px; font-size:12px; color:#777; display:flex; align-items:center; gap:4px; cursor:pointer; border-bottom:2px solid transparent; white-space:nowrap;` + hover | `px-2 py-1 text-[12px] text-[#777] flex items-center gap-1 cursor-pointer border-b-2 border-transparent whitespace-nowrap hover:text-[#bbb] hover:bg-[#222240]` |
| `.file-tab.active` | `color:#e0e0e0; background:#1a1a2e; border-bottom-color:#80a0ff;` | `text-[#e0e0e0] bg-[#1a1a2e] border-b-[#80a0ff]` |
| `.file-tab-icon` | `font-size:13px;` | `text-[13px]` |
| `.file-tab-name` | `max-width:150px; overflow:hidden; text-overflow:ellipsis;` | `max-w-[150px] overflow-hidden text-ellipsis` |
| `.file-tab-close` | `font-size:10px; color:#555; padding:0 2px; border-radius:2px; line-height:1;` + hover | `text-[10px] text-[#555] px-0.5 rounded-[2px] leading-none hover:text-[#ff6060] hover:bg-[#3a2020]` |
| `.file-content` | `flex:1; overflow:auto; padding:12px; font-family:'JetBrains Mono','Fira Code',monospace; font-size:12px; line-height:1.6; color:#c8c8e0; background:#1a1a2e; white-space:pre; margin:0;` | `flex-1 overflow-auto p-3 font-mono text-[12px] leading-[1.6] text-[#c8c8e0] bg-[#1a1a2e] whitespace-pre m-0` |
| `.file-content-empty` | `display:flex; align-items:center; justify-content:center; height:100%; color:#666;` | `flex items-center justify-center h-full text-[#666]` |
| `.file-content-error` | `padding:12px; color:#ff6060; font-weight:bold;` | `p-3 text-[#ff6060] font-bold` |
| `.file-content-loading` | `display:flex; align-items:center; justify-content:center; height:100%; color:#888;` | `flex items-center justify-center h-full text-[#888]` |

### Agents Panel

| Class | Current CSS | Tailwind Equivalent |
|---|---|---|
| `.agents-panel` | `flex:1; overflow-y:auto; padding:8px;` | `flex-1 overflow-y-auto p-2` |
| `.agents-panel-loading/.empty/.error` | `display:flex; align-items:center; justify-content:center; height:100%; color:#666; padding:20px; text-align:center;` | `flex items-center justify-center h-full text-[#666] p-5 text-center` |
| `.agents-panel-error` | `color:#ff6060;` | `text-[#ff6060]` |
| `.agent-item` | `border-bottom:1px solid #2a2a44;` | `border-b border-[#2a2a44]` |
| `.agent-item-header` | `display:flex; align-items:center; padding:8px 10px; cursor:pointer; gap:8px;` + hover | `flex items-center px-2.5 py-2 cursor-pointer gap-2 hover:bg-[#222240]` |
| `.agent-item-chevron` | `font-size:10px; color:#666; transition:transform 0.15s;` | `text-[10px] text-[#666] transition-transform duration-150` |
| `.agent-item-name` | `font-weight:600; font-size:13px; color:#e0e0e0;` | `font-semibold text-[13px] text-[#e0e0e0]` |
| `.agent-item-scope` | `font-size:10px; padding:1px 6px; border-radius:3px; font-weight:bold; margin-left:auto;` | `text-[10px] px-1.5 py-0.5 rounded-[3px] font-bold ml-auto` |
| `.agent-item-desc` | `font-size:12px; color:#888; padding:0 10px 6px 28px;` | `text-[12px] text-[#888] px-2.5 pb-1.5 pl-7` |
| `.agent-item-detail` | `padding:8px 10px 8px 28px; font-size:12px; background:#16162a;` | `px-2.5 pb-2 pl-7 text-[12px] bg-[#16162a]` |
| `.agent-detail-row` | `padding:2px 0;` | `py-0.5` |
| `.agent-detail-label` | `color:#6090ff; font-weight:600;` | `text-[#6090ff] font-semibold` |
| `.agent-detail-value` | `color:#ccc; font-family:monospace;` | `text-[#ccc] font-mono` |

### Sessions Panel

| Class | Current CSS | Tailwind Equivalent |
|---|---|---|
| `.sessions-panel` | `flex:1; overflow-y:auto; padding:8px;` | `flex-1 overflow-y-auto p-2` |
| `.sessions-panel-header` | `padding:4px 10px 8px; font-size:12px; font-weight:600; color:#888; text-transform:uppercase; letter-spacing:0.5px;` | `px-2.5 pt-1 pb-2 text-[12px] font-semibold text-[#888] uppercase tracking-[0.5px]` |
| `.sessions-panel-loading/.empty/.error` | Same as agents panel | Same Tailwind |
| `.session-item` | `display:flex; align-items:center; padding:8px 10px; border-bottom:1px solid #2a2a44; cursor:pointer; gap:8px;` + hover | `flex items-center px-2.5 py-2 border-b border-[#2a2a44] cursor-pointer gap-2 hover:bg-[#222240]` |
| `.session-item-id` | `font-family:monospace; font-size:13px; color:#e0e0e0; font-weight:600; min-width:80px;` | `font-mono text-[13px] text-[#e0e0e0] font-semibold min-w-[80px]` |
| `.session-item-count` | `font-size:11px; color:#888;` | `text-[11px] text-[#888]` |
| `.session-item-age` | `font-size:11px; color:#666; margin-left:auto;` | `text-[11px] text-[#666] ml-auto` |
| `.session-resume-btn` | `padding:3px 10px; background:#408040; color:#e0e0e0; border:none; border-radius:3px; cursor:pointer; font-size:12px; margin-left:4px; flex-shrink:0;` + hover + disabled | `px-2.5 py-0.5 bg-[#408040] text-[#e0e0e0] border-none rounded-[3px] cursor-pointer text-[12px] ml-1 flex-shrink-0 hover:bg-[#50a050] disabled:bg-[#333355] disabled:cursor-not-allowed` |

### Session Overlay

| Class | Current CSS | Tailwind Equivalent |
|---|---|---|
| `.session-overlay` | `position:fixed; top:0; left:0; right:0; bottom:0; background:rgba(0,0,0,0.7); z-index:200; display:flex; align-items:center; justify-content:center;` | `fixed inset-0 bg-black/70 z-[200] flex items-center justify-center` |
| `.session-detail-panel` | `background:#1a1a2e; border:1px solid #333355; border-radius:8px; width:80vw; max-width:900px; height:70vh; display:flex; flex-direction:column; overflow:hidden;` | `bg-[#1a1a2e] border border-[#333355] rounded-lg w-[80vw] max-w-[900px] h-[70vh] flex flex-col overflow-hidden` |
| `.session-detail-header` | `display:flex; align-items:center; justify-content:space-between; padding:8px 12px; border-bottom:1px solid #2a2a44; font-family:monospace; font-size:13px; color:#e0e0e0;` | `flex items-center justify-between px-3 py-2 border-b border-[#2a2a44] font-mono text-[13px] text-[#e0e0e0]` |
| `.session-close-btn` | `background:none; border:none; color:#888; font-size:16px; cursor:pointer; padding:2px 6px; border-radius:3px;` + hover | `bg-none border-none text-[#888] text-[16px] cursor-pointer px-1.5 py-0.5 rounded-[3px] hover:text-[#ff6060] hover:bg-[#2a1a1a]` |
| `.session-detail-loading` | `display:flex; align-items:center; justify-content:center; flex:1; color:#666;` | `flex items-center justify-center flex-1 text-[#666]` |
| `.session-detail-entries` | `flex:1; overflow-y:auto; padding:8px;` | `flex-1 overflow-y-auto p-2` |

### Status Colors

| Class | Current CSS | Tailwind Equivalent |
|---|---|---|
| `.status-running` | `color:#f0c040;` | `text-[#f0c040]` |
| `.status-idle` | `color:#80c080;` | `text-[#80c080]` |
| `.unsafe-mode` | `color:#ff4040; font-weight:bold;` | `text-[#ff4040] font-bold` |

### Keyframes (Custom CSS via @theme or custom layer)

| Animation | CSS | Tailwind Equivalent |
|---|---|---|
| `conn-pulse` | `@keyframes conn-pulse { 0%,100%{opacity:1;} 50%{opacity:0.3;} }` | `animate-pulse` (built-in) — or define custom in `@utility` |
| `conn-blink` | `@keyframes conn-blink { 0%,100%{opacity:1;} 50%{opacity:0.2;} }` | Custom `@utility animate-conn-blink` needed |

### Responsive Breakpoints

| Breakpoint | Current CSS | Tailwind Approach |
|---|---|---|
| `<= 1024px` | `.sidebar { width:33.33%; min-width:200px; }`, `.tab { padding:6px 12px; font-size:12px; }` | `lg:w-[240px] lg:min-w-[180px] w-[33.33%] min-w-[200px]` |
| `<= 768px` | `.sidebar { width:33.33%; min-width:160px; }`, `.modal-content { min-width:auto; width:90vw; max-width:500px; }` | `md:w-[33.33%] md:min-w-[160px]` |
| `<= 480px` | `.status-bar { font-size:10px; padding:3px 8px; }`, `.tab-bar { overflow-x:auto; }`, `.tab { padding:6px 8px; font-size:11px; white-space:nowrap; }`, `.sidebar { width:40%; min-width:120px; }` | `sm:text-[10px] sm:px-2 sm:py-0.5`, `sm:overflow-x-auto`, etc. |

Note: Tailwind's default breakpoints are `sm:640px`, `md:768px`, `lg:1024px`, `xl:1280px`. Our breakpoints map to `sm:480px`, `md:768px`, `lg:1024px`. Since Tailwind uses min-width (mobile-first) and our existing CSS uses max-width (desktop-first), we invert the logic: the base class is the smallest-screen style, and `sm:`, `md:`, `lg:` prefixes override for larger screens.

**Concrete responsive example — sidebar width:**
- Existing: base = `width:240px`, `@media (max-width:1024px)` = `width:33.33%`, `@media (max-width:768px)` = `width:33.33%`, `@media (max-width:480px)` = `width:40%`
- Tailwind: `class: "w-[40%] sm:w-[33.33%] md:w-[33.33%] lg:w-[240px]"`

The base class `w-[40%]` applies at all sizes, then `sm:` overrides at 480px+, `md:` at 768px+, `lg:` at 1024px+.

## Error Handling

- If `npx @tailwindcss/cli` fails (network error, Node.js unavailable), `rebuild-web.sh` exits with non-zero and prints the error. No partial dist directory is produced.
- If a Tailwind class name is misspelled, it is silently ignored by Tailwind (no runtime error, just missing styles). This is the same as current CSS — a typo in a class name just does nothing.

## Testing

1. **Visual regression:** Compare the UI before and after migration by loading both versions side-by-side in a browser.
2. **Responsive testing:** Resize browser to 480px, 768px, and 1024px — verify layout adapts as expected.
3. **Build verification:** `scripts/rebuild-web.sh` completes without errors on a clean workspace.
