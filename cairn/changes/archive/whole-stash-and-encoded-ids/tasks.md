---
cairn: tasks
change: whole-stash-and-encoded-ids
---

- [x] Reproduce in the crate: a 3,000-octet unmanaged line at the top level and inside a stashed component, a stash over the budget, every id-taking request with an id holding the separators of a URL, an id naming no segment, an entity tag holding a control character, a trigger overflowing its seconds
- [x] Stash a line whatever its width
- [x] Give the stash a budget, and return what does not fit beside the event
- [x] Build every request URL from its segments, refuse the segments a URL drops
- [x] Refuse an entity tag holding a control character
- [x] Check the arithmetic of `lead_minutes`
- [x] CHANGELOG.md, fold into cairn/spec/events-ical.md and cairn/spec/requests.md, log, archive
- [ ] Check live the total Google accepts in private extended properties, and how it counts (not done: no live run in this change)
- [ ] Refuse a control character in the bearer token, in `content_type` and in `method` (not done: it waits on the check io-http is to make in its request, or it turns four more builders into `Result`)
- [ ] Drop the id encoding the consumers apply themselves (not done: other repositories)
