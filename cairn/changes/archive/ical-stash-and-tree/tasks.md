---
cairn: tasks
change: ical-stash-and-tree
---

- [x] Reproduce in the crate: a folded unmanaged line (property and nested), a folded EXDATE, a stash stored with its folds, each marker in each text field, a round-trip sweep over generated lines
- [x] Write the logical line to the stash and to `recurrence`, a stashed component as its logical lines
- [x] Read both stash shapes in `stashed`
- [x] Build the document as one tree in `to_ical_series`, `to_ical` on top of it, no string splice left
- [x] Mint the zones once per document, from the tree
- [x] CHANGELOG.md, fold into cairn/spec/events-ical.md, log, archive
- [ ] Cache the VTIMEZONE across a listing (not done: needs a public cache type and a key for the era ical-rs does not expose)
- [ ] `Accept-Encoding: gzip` and the `fields` parameter (not done: the first waits on io-http, the second is a feature of its own)
- [ ] Check live what Google does to a CR and to a leading or trailing whitespace in an extended property value (not done: no live run in this change)
