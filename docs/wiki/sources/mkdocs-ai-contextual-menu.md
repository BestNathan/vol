---
type: source
source_type: code
date: 2026-08-27
ingested: 2026-08-27
tags: [documentation, github-pages, mkdocs, ai-context, chatgpt, claude]
---

# MkDocs AI Contextual Menu

**Authors/Creators:** BestNathan (ChatGPT-assisted)
**Date:** 2026-08-27
**Link:** `.github/pages/mkdocs.yml`, `.github/pages/requirements.txt`

## TL;DR

The Vol Wiki now exposes page-level AI handoff actions through a pinned MkDocs plugin.
Readers can copy clean Markdown, copy or view the canonical raw Markdown URL, and open a
ChatGPT or Claude conversation that points to the current page.

## Key Takeaways

- `mkdocs-copy-to-llm==0.2.10` is pinned for reproducible Pages builds.
- Every page provides Copy page, Copy Markdown link, View as Markdown, Open in ChatGPT,
  and Open in Claude actions.
- The raw source root is
  `https://raw.githubusercontent.com/BestNathan/vol/main/docs/wiki`.
- `base_path: /vol` strips the project Pages prefix before resolving the source path.
- Analytics is explicitly disabled; production assets remain minified.
- The Material theme language is English, matching the repository documentation.

## Detailed Summary

The existing MkDocs Material site already rendered `docs/wiki` with strict link checking,
roam-style wikilinks, automatic navigation, and search. The contextual-menu integration adds
an AI-oriented handoff layer without changing that content model.

The primary copy action fetches the canonical Markdown source and copies it without rendered
navigation or theme chrome. Secondary actions expose the raw URL or create a new ChatGPT or
Claude conversation with a prompt asking the assistant to read that public Markdown URL.

Because the Pages site is hosted at `/vol/` while the Markdown files live below
`docs/wiki/`, the plugin must use both the nested raw repository URL and `base_path: /vol`.
Without that mapping, a rendered route such as `/vol/concepts/tool-registry/` would resolve
to a nonexistent repository-root path instead of
`docs/wiki/concepts/tool-registry.md`.

## Entities Mentioned

- [[vol-repository]]: hosts the MkDocs Pages workflow and the canonical wiki sources.

## Concepts Covered

No new architectural concept page was introduced; this is a repository documentation
integration recorded as a source.

## Notes

- ChatGPT and Claude actions pass the public Markdown URL rather than embedding the full page
  in a query string.
- The Pages pull-request build remains the integration gate for plugin installation, strict
  MkDocs rendering, assets, and link resolution.
