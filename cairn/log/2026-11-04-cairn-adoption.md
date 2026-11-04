---
cairn: log
change: cairn-adoption
landed: 2026-11-04
---

# Adopt Cairn, retiring docs/

This repository now keeps cairn/spec/ (current truth, one file per capability), cairn/changes/ (in-flight proposals) and cairn/log/ (dated history), with the cairn/ directory as the root marker, AGENTS.md (plus CLAUDE.md) as the activation stanza and cairn/verify.sh, vendored from io-gmail, as the optional checker. docs/ was deleted and CONTRIBUTING.md now points at cairn/.

docs/sync.md moved to cairn/spec/sync.md with its prose unchanged, after checking its two claims about the crate against the code: `showDeleted` is only put on the wire when true (`skip_serializing_if = "is_false"`), and `GcalSendError::is_sync_token_expired` recognises the HTTP 410. docs/README.md, an index of that one file, was dropped. Other capabilities get a file the first time a change touches them. History before this entry lives in CHANGELOG.md and the git log, up to the dependency bump (ical-rs 0.7, io-http 0.6, Rust 1.89) of 2026-11-04.

Capabilities moved: sync (ADDED, moved from docs/). This change moved documentation, not behaviour.
