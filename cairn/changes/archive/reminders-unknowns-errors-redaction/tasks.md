---
cairn: tasks
change: reminders-unknowns-errors-redaction
---

- [x] Reproduce in the crate: reminders off through a round trip, an unknown value in each of the 18 enums, a page of events and a calendar list holding one, a rate limit answered with a 403, a 410 for a deleted resource, the debug form of a channel and of an entry point
- [x] Keep `useDefault` of the server copy in `merge` when the document holds no VALARM
- [x] Add `Unknown(String)` to the 18 enums, take such a value from the server copy in `merge`
- [x] Port `GcalApiError`, tell a deleted resource from an expired token in `is_sync_token_expired`
- [x] Redact the token of a channel and the codes of an entry point in `Debug`
- [x] CHANGELOG.md, src/lib.rs, fold into cairn/spec (events-ical, requests, sync, resources), log, archive
- [ ] Mint a VALARM for the default reminders of the calendar, so a local client rings for an event on the defaults (not done: needs the calendar list entry at projection time, a new input)
- [ ] Check live the reasons Calendar gives with a 410 and with a rate limit (not done: no live run in this change)
- [ ] Make the integer fields (`sequence`, `minutes`, `additionalGuests`) survive a value out of range (not done: outside the four findings)
