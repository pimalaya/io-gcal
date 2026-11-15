---
cairn: change
id: series-exceptions-written-through-instances
status: landed
created: 2026-11-15
---

# Series exceptions written through instances

## Why

The live testing campaign of 2026-11-15 found the override of a recurring series lost on its way to Google, with no warning (.github cairn changes/live-testing-2026-11-15, reports/calendula: gcal-google.md G-F3, part2-pimdir.md P2-F6):

- `calendula -a google event create` of a series holding an override (a RECURRENCE-ID moving 2026-11-09 09:00 to 2026-11-10 15:00) sends one `events.import`; `gcal instances` then shows the occurrence at its rule's time. The EXDATE of the same document is kept.
- `neverest sync -s gcal` pushes a series the same way: one import, none for the exception. The store keeps the override, so store and server disagree with nothing said.

Both go through `GcalEvent::from_ical`, which is where the override stops existing:

- src/v3/rest/events/ical.rs:786, `take_vevent` takes the VEVENT replacing no instance as the event, and the document's other VEVENTs are read by nothing else;
- src/v3/rest/events/ical.rs:818, `calendar_remainder` skips every VEVENT, so an override does not even reach the calendar stash.

neverest has since pushed the overrides on its own side (776fca9, its change a-series-is-pushed-whole), reading each as a document of its own and merging it onto its instance; calendula's direct create still goes through the master-only projection, so the crate gives the series reading both can share.

An EXDATE is not lost: `consume_prop` (ical.rs:919) keeps it among the `recurrence` lines, which `events.import` takes as they are. The module header already says one occurrence is written through its instance, from a document holding it alone, `merge` keeping the instance the server copy names; what is missing is the step from a series document to those instance writes.

## What

Under `ical`, in src/v3/rest/events/ical.rs:

- **`GcalEventSeries`**, a series document cut the way Google takes it: `master`, the event `from_ical` reads, its EXDATEs in its `recurrence`; `instances`, each override read as a document holding it alone, its `original_start_time` naming its instance, to write once the master is; `left_out`, every unit Google cannot hold, as its logical lines, never dropped silently.
- **`GcalEventSeries::from_ical(contents)`**: the master with the calendar remainder, each other VEVENT without it. A stash past the budget does not fail the series: the units it leaves out join `left_out`, the event keeping every unit that fits, as `GcalEventIcalError::StashFull` carries them for one event.
- **`GcalEvent::replaces(&self, instance)`**: whether an instance `events.instances` lists (or an exception a listing returns) is the occurrence this override replaces, its `originalStartTime` the same instant, or the same date in an all-day series.
- **`GcalEvent::instance_window(&self)`**: the `timeMin` and `timeMax` of an `events.instances` listing holding that instance, from the day before its original start to two days after, at midnight UTC.
- **What `left_out` names**: an override whose RECURRENCE-ID carries a RANGE (Google changes one instance at a time), one whose RECURRENCE-ID does not read, an override beside an event that is no series, and the units a stash goes without.

A caller imports or writes the master, then, for each instance, finds the listed instance it `replaces`, merges the override onto it (`merge`, which keeps the instance's id and original start and its provider-only fields) and updates it, and warns of or refuses `left_out`. An override whose STATUS is CANCELLED cancels its instance by that update, as Google holds a cancelled occurrence.

## Scope / non-goals

- No coroutine: `events.import`, `events.instances`, `events.update` and `events.patch` exist. The instance id Google mints (`<id>_<original start in UTC>`) and the `originalStart` parameter of `events.instances` would spare the listing, but neither is verified live here, so the crate matches on the listed `originalStartTime`.
- An exception the server holds and the document dropped is not reverted: Google keeps an instance it was told about, and putting it back to the series needs the series carried to that occurrence. Its document reads it back on the next read, so it comes back rather than being lost; left as a follow-up.
- No edit to calendula or neverest: their callers keep `from_ical` until they move (notes in the log entry).
- No release: left to the owner.
