---
cairn: log
change: series-exceptions-written-through-instances
landed: 2026-11-15
---

# Series exceptions written through instances

The live testing campaign of 2026-11-15 (.github cairn changes/live-testing-2026-11-15, reports/calendula/gcal-google.md G-F3 and part2-pimdir.md P2-F6) found the override of a series dropped on its way to Google, through `calendula event create` and through `neverest sync -s gcal`: one `events.import` of the master, the EXDATE kept in its recurrence, nothing for the override, and no warning. The override stopped existing in this crate's projection: `GcalEvent::from_ical` reads the VEVENT `take_vevent` picks (src/v3/rest/events/ical.rs:786 before the change), the master, and `calendar_remainder` (ical.rs:818 before the change) skips every VEVENT, so no other component reached the event or its stash. neverest has since pushed a series whole on its own side (776fca9, its change a-series-is-pushed-whole), reading each override as a document of its own and updating its instance with `from_ical` and `merge`; calendula's direct create still imports the master alone.

Under `ical`, `GcalEventSeries::from_ical(contents)` now reads a series document as `master`, the event `from_ical` reads with the calendar remainder, its EXDATEs among its recurrence lines as before, `instances`, every other VEVENT read as a document holding it alone with no calendar remainder (`from_ical` now reads one VEVENT through the private `from_vevent`), and `left_out`, as logical lines: an override whose RECURRENCE-ID carries a RANGE or does not read, an override beside an event that is no series, and the units a stash past the budget goes without, which no longer fail the read. `GcalEvent::replaces` tells whether a listed instance (or an exception a listing returns) is the occurrence an override replaces, both original starts compared as dates or as instants, an offset-less time read in its zone; `GcalEvent::instance_window` gives the `timeMin` and `timeMax` of an `events.instances` listing holding it, the day before to two days after. A caller merges each instance onto the listed one (`merge` keeping its id, its original start and its provider-only fields) and updates it; an override whose STATUS is CANCELLED cancels its instance by that update. No coroutine was added: import, instances, update and patch exist.

Tests written first and seen failing on a stub reproducing the old projection (master alone, `instances` and `left_out` empty): `a_series_carries_its_override_to_its_instance` ("one instance: []"), `an_instance_finds_the_occurrence_it_replaces`, `what_google_cannot_hold_of_a_series_is_named` (left 0, right 1), and in tests/events.rs `writes_a_series_with_its_instances`, which imports the master (its body carrying the EXDATE), lists the instances on a scripted answer, and asserts the `PUT /calendar/v3/calendars/primary/events/m1_20261124T130000Z` with `If-Match: "e2"`, the new start, the listed `originalStartTime`, `recurringEventId` and the provider-only `colorId` kept. All pass after the change, with the whole suite, clippy `-D warnings` and the no_std build. The live `ical_series_with_its_instances` (tests/google.rs) was written and not run: this session had no account.

Left:

- Run `ical_series_with_its_instances` with the service account.
- An exception the server holds and a document dropped is not reverted to the series (proposal, scope).
- calendula: src/gcal/backend.rs `create_item` (lines 291-319) and `update_item` (lines 322-350) move to `GcalEventSeries`: `project` (lines 385-399) reads the series and warns of `left_out` as it warns of a full stash today, the master is inserted, imported or updated as now, then each instance is merged onto the listed one it `replaces`, found in `instance_window` through `event_instances` (or among `exceptions()` on an update), and updated with that instance's etag.
- neverest needs nothing to work: 776fca9 pushes the overrides. It could later trade `Occurrences` (src/kind/event.rs) and its string match `names_occurrence` for `GcalEventSeries`, `replaces` and `instance_window`, which also match an override named in another zone than the series and keep the calendar remainder out of the instances' stash; its RANGE refusal reads `left_out`.
- The release, to the owner.

Capabilities moved: events-ical ("A series is written with its instances", "An instance names the occurrence it replaces", "What Google cannot hold of a series is named").
