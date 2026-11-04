---
cairn: change
id: cairn-adoption
status: landed
created: 2026-11-04
---

# Adopt Cairn, retiring docs/

## Why

docs/ held one living note (sync.md, how sync tokens and push channels compose) behind an index README, outside any convention: nothing told a reader it was current truth, and no place existed for proposals or a dated history a change can state a delta against.

## What

Keep the three Cairn artifacts next to the code: cairn/spec/ for current truth, cairn/changes/ for in-flight proposals and cairn/log/ for the dated history, with the cairn/ directory as the root marker, AGENTS.md (plus CLAUDE.md) as the activation stanza and cairn/verify.sh as the optional checker. docs/sync.md becomes the first capability, cairn/spec/sync.md, its prose unchanged; docs/README.md, an index of that one file, is dropped with docs/. CONTRIBUTING.md points at cairn/.
