---
cairn: tasks
change: series-exceptions-written-through-instances
---

- [x] Regression tests first, failing on the master-only projection: a series with one EXDATE and one override yields the master, its EXDATE in its recurrence, and one instance; the instance a listing names, matched by its original start whatever its offset, and the update merged onto it; a RANGE, an unreadable RECURRENCE-ID, an override beside a lone event and a full stash named in `left_out`; the requests a caller sends, on the scripted coroutines
- [x] `GcalEventSeries`, `GcalEvent::replaces`, `GcalEvent::instance_window`, under `ical`
- [x] Docs: the module header, CHANGELOG.md (Added)
- [x] `cargo test --all-features`, `cargo clippy --all-features -- -D warnings`, `cargo fmt`, `cargo build --no-default-features`
- [x] Fold into cairn/spec/events-ical.md, log, archive
- [ ] Live test with the service account: `ical_series_with_its_instances` (written, not run; left to the owner, no live account in this session)
- [ ] An exception the document dropped reverted to the series (follow-up)
- [ ] calendula moves to `GcalEventSeries`; neverest may (notes in the log entry)
- [ ] Release, left to the owner
