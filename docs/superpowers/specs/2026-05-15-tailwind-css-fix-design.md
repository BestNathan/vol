# Design Spec: Fix TailwindCSS for Dioxus Web UI

## Background

The `input.css` at `crates/vol-llm-ui/src/web/input.css` referenced TailwindCSS v4 (`@import "tailwindcss"`), but the compiled CSS was never injected into the HTML document. All components use `class` attributes with Tailwind utility classes, but none render visually.

## Root Cause

Three issues:
1. The `asset!()` macro from `manganis` requires the dioxus "asset" feature, which was not enabled
2. No `document::Stylesheet` component was used to inject the CSS into the DOM
3. The old `input.css` and `index.html` were in `src/web/` — a non-standard location

## Fix Applied

1. **Enable dioxus asset feature**: Added `dioxus?/asset` to the `web` feature gate in `crates/vol-llm-ui/Cargo.toml`
2. **Move CSS to crate assets**: Created `crates/vol-llm-ui/assets/tailwind.css` with corrected `@source` path (`../src/web/components/*.rs`)
3. **Inject stylesheet**: Added `document::Stylesheet { href: asset!("/assets/tailwind.css") }` at the top of `App` component's RSX
4. **Delete old files**: Removed `crates/vol-llm-ui/src/web/input.css` and `crates/vol-llm-ui/src/web/index.html`

## How `asset!()` Works

The `asset!("/path/to/file")` macro resolves relative to `CARGO_MANIFEST_DIR` (the crate root). So `asset!("/assets/tailwind.css")` maps to `crates/vol-llm-ui/assets/tailwind.css`. The `dx serve` CLI detects the CSS file and runs Tailwind CSS CLI to compile it.

## Files Changed

- Create: `crates/vol-llm-ui/assets/tailwind.css`
- Modify: `crates/vol-llm-ui/Cargo.toml` (add `dioxus?/asset` to web feature)
- Modify: `crates/vol-llm-ui/src/web/components/app.rs` (add `document::Stylesheet`)
- Delete: `crates/vol-llm-ui/src/web/input.css`
- Delete: `crates/vol-llm-ui/src/web/index.html`

## Verification

```bash
cargo check -p vol-llm-ui --features web --no-default-features
```
