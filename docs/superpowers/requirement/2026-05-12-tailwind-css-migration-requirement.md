# Requirements: Tailwind CSS Migration for vol-llm-ui Web Frontend

## Background
The current web frontend uses a single `GLOBAL_CSS` string (~200 lines, ~100 classes) embedded in `app.rs`. CSS is injected via `<style>` tag. Existing media queries (1024/768/480px) provide minimal responsive behavior — layout never stacks vertically. The user wants full Tailwind CSS utility-class styling with responsive breakpoints.

Note: `tailwind-rs-dioxus` crate exists but depends on Dioxus 0.3 (2022), incompatible with our Dioxus 0.7.

## Goals
1. Replace all `GLOBAL_CSS` embedded styling with Tailwind utility classes in rsx! `class:` attributes
2. Add proper responsive breakpoints so the UI works on mobile (480px), tablet (768px), and desktop (1024px+)
3. Use `npx tailwindcss` CLI to generate CSS, included via `<link>` in `index.html`
4. No regression in existing visual appearance — same colors, spacing, layout patterns, just expressed as Tailwind classes

## Non-Goals
- Do NOT use `tailwind-rs-dioxus` or any Rust Tailwind integration crate (incompatible with Dioxus 0.7)
- Do NOT change the JavaScript/WASM build pipeline — CSS is separate static asset
- Do NOT redesign the UI layout — preserve existing layout, only change how CSS is expressed
- Do NOT add new features beyond responsive behavior

## Scope
**Included:**
- Install `@tailwindcss/cli` (Tailwind CSS v4)
- Create `input.css` with `@theme` directives for custom colors/fonts/breakpoints and `@source` directives pointing to Rust component files
- Migrate all ~100 CSS classes from `GLOBAL_CSS` (spread across ~15 component files) to Tailwind utility classes in rsx! components
- Update `index.html` to link generated CSS
- Update `scripts/rebuild-web.sh` to run Tailwind CLI before serving

**Excluded:**
- TUI changes (this is web-only)
- Backend/API changes
- New UI features or layout changes

## Constraints
- Must work with Dioxus 0.7 web build (wasm32 target)
- Node.js v24 is available on the target machine
- The build must be reproducible — CI-friendly (script-based, not interactive)
- Tailwind CSS v4 uses different config format than v3 — use v4 syntax

## Success Criteria
1. `npx @tailwindcss/cli -i src/web/input.css -o target/wasm32-unknown-unknown/wasm-dev/dist/tailwind.css` produces valid CSS
2. All existing visual styles are preserved — verify by checking: app layout, status bar, tab bar, file tree, conversation messages (user/assistant/tool), modal dialogs, session panel, tools panel, agents panel, workspace view, log viewer, skills table, approval dialog
3. UI is usable on a 480px screen — sidebar collapses or goes full-width, tabs are accessible
4. No Rust compilation errors in any component after migration
5. `scripts/rebuild-web.sh` completes without manual steps

## Edge Cases
1. **Dynamic inline styles** — some components use inline `style:` for dynamic values (file tree indent, tool result colors). These must remain as inline styles, not converted to Tailwind classes.
2. **State-based classes** — `.active`, `.collapsed`, `.selected` states currently handled via CSS. Need to map to Tailwind's conditional class approach or use `@apply` in custom CSS.
3. **Custom animations** — if any CSS uses transitions/animations not in Tailwind defaults, define as custom `@utility` or keep in custom CSS layer.
4. **Cross-origin fonts** — current CSS imports Google Fonts. Tailwind doesn't bundle fonts — keep the `@import` in input.css.

## Decisions
- **Tailwind CSS v4** — uses new CSS-first config format (no `tailwind.config.js` needed, everything in `input.css`)
- **CSS purging** — Tailwind v4 only generates utilities that are referenced in scanned content files, so bundle size is automatically minimal
