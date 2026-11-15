---
cairn: tasks
change: boundaries-in-the-event-zone
---

- [x] Regression tests first, seen failing on the old projection: the weekly series of G-F2 from a calendar in UTC, in New York and in Paris, projected `DTSTART;TZID=Europe/Paris:20270315T090000` with its RRULE and expanded at 09:00 in Paris after the change of 28 March; its override projected `RECURRENCE-ID;TZID=Europe/Paris:20270329T090000`, replacing the third occurrence and the listed instance; a zone the database does not know keeping the instant
- [x] `push_boundary` writes the wall time of the named zone (`wall_time`, `has_offset`), the UTC instant for a zone the database does not know
- [x] The two tests pinning the display-zone premise rewritten to assert the instant and the zone; the merge of a document holding UTC stamps still takes the server's zone
- [x] Docs: the module header, `push_boundary`, `ical_local`, `carry_zone` (was `carry_display_zone`), CHANGELOG.md (Fixed)
- [x] `cargo fmt`, `cargo test --features ical`, `cargo test --all-features`, `cargo clippy --all-features --all-targets -- -D warnings`, `cargo build --no-default-features`, `cargo build --no-default-features --features ical`
- [x] Fold into cairn/spec/events-ical.md, log, archive
- [ ] Live check: E7 (or a new weekly event in Paris in a calendar in UTC) read after the bump, and one event in a second zone in a calendar neither in UTC nor in the event's zone (audit GCAL-03, scenario 2); left to the owner, no live run in this session
- [ ] calendula: rewrite its projection.md "An instant outranks a display zone" when it moves to the release carrying this fix
- [ ] Release, left to the owner
