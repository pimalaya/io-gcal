---
cairn: log
change: cargo-006-features
landed: 2026-11-21
---

# The layered feature shape

The crate moved to the family's feature shape (GUIDELINES.md cargo-006), the one io-imap, io-smtp and io-webdav ship. Until now `client = []` gated the std-blocking client and the coroutines were always compiled.

- `client`, on by default, now gates the coroutines: `coroutine`, `v3::send`, `v3::query` and every method submodule of `v3::rest`. It pulls the dependencies only they need: io-http, log, thiserror and url (log also comes with `ical`).
- `std`, implying `client`, gates the std-blocking client (`v3::client`) and `extern crate std`.
- Each TLS feature implies `std`, selects its pimalaya-stream provider and pulls anyhow and pimalaya-stream; `vendored` forwards to the weak pimalaya-stream dependency.

Without `client` the crate keeps the no_std resource types of `v3::rest` and the `ical` bridge. Both breaking renames are in the CHANGELOG with the before and after of a consumer's dependency line. The CI matrix (the shared Nix tests workflow, the live jobs) names no moved feature and stays as is.

No change folder: the move applies a family rule and changes no behaviour.

Capabilities moved: none.
