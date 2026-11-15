---
cairn: log
change: boundaries-in-the-event-zone
landed: 2026-11-15
---

# Boundaries in the event's zone

The live testing campaign of 2026-11-15 (.github cairn changes/live-testing-2026-11-15, reports/calendula/gcal-google.md G-F2) found a weekly event written at 09:00 in Europe/Paris, in a calendar whose zone is UTC, projected as `DTSTART:20270315T080000Z` with its RRULE and no TZID: Google returned `{"dateTime":"2027-03-15T08:00:00Z","timeZone":"Europe/Paris"}` and expanded the series in Paris (`2027-03-29T07:00:00Z`), while a client expanding the projection listed 10:00 in Paris after the change of 28 March. The io-family audit of 2026-11-13 had raised the same branch and a second one (GCAL-03).

Root cause, confirmed by reading and by the tests below: `push_boundary` (src/v3/rest/events/ical.rs:1305 before the change) took the TZID path only for a `dateTime` not stamped `Z`, and wrote its literal digits (`ical_local` drops the offset), while a `Z` stamp fell through to `ical_utc`, on the premise that `timeZone` is the calendar's display zone and that a zoned boundary comes in its zone's own offset. `timeZone` is the event's own zone, the one Google expands a series in, and `dateTime` is rendered in the calendar's zone: a calendar in UTC gives the instant with a `Z`, one in New York gives `2027-03-15T04:00:00-04:00`, which the old code wrote as `DTSTART;TZID=Europe/Paris:20270315T040000`.

`push_boundary` now writes a timed boundary whose `timeZone` names a zone other than UTC as its wall time in that zone, with its TZID: `wall_time` reads a `dateTime` carrying an offset (`has_offset`, `Z` included) as an instant and places it on the zone's clock through jiff's bundled database, the one `instant` and the VTIMEZONE synthesis already read, and takes a `dateTime` without offset as the wall time it is. A zone the database does not know keeps the instant in UTC (a `Z` stamp did so before; a `dateTime` in another offset was written under a TZID no VTIMEZONE could define), and keeps the wall time of a `dateTime` without offset under its TZID. A `timeZone` of UTC or none and an all-day `date` project as before. DTSTART, DTEND and the RECURRENCE-ID of `originalStartTime` share the path, so the fix covers all three; EXDATE and RDATE are recurrence lines spliced back as Google returns them and never went through it. `replaces` compares original starts as instants and `instance_window` keeps a day of margin each side, so `GcalEventSeries` reads the zoned RECURRENCE-ID a series document now carries with no change. `carry_display_zone` is renamed `carry_zone`, its behaviour kept: `merge` still gives a UTC-stamped boundary of the document the server's `timeZone`, which a document projected before this change, still in a store, needs so its write does not move Google's expansion to UTC.

Tests written first and seen failing on the old projection:

- `a_stamp_reads_as_wall_time_in_the_zone_google_names`, the G-F2 series rendered from calendars in UTC, New York and Paris, each projected `DTSTART;TZID=Europe/Paris:20270315T090000`, `DTEND;TZID=Europe/Paris:20270315T100000`, the RRULE and the Europe/Paris VTIMEZONE, and expanded through ical-rs's `IcalRecurTimeline` to 2027-03-29 and 2027-04-05 at 07:00 UTC. Before: `2027-03-15T08:00:00Z: no "DTSTART;TZID=Europe/Paris:20270315T090000\r\n"`, the document holding `DTSTART:20270315T080000Z`, `DTEND:20270315T090000Z` and `RRULE:FREQ=WEEKLY;COUNT=4`.
- `an_override_names_its_occurrence_in_the_zone_of_its_series`, its 29 March instance moved to 11:00, both from a calendar in UTC: `RECURRENCE-ID;TZID=Europe/Paris:20270329T090000`, the override in place of the third occurrence, read back by `GcalEventSeries::from_ical` as an instance that `replaces` the listed one. Before: no such line, the document holding `RECURRENCE-ID:20270329T070000Z`.
- `a_zone_the_database_does_not_know_keeps_the_instant`. Before: `2027-03-15T10:00:00+02:00` under `Mars/Olympus_Mons` projected `DTSTART;TZID=Mars/Olympus_Mons:20270315T100000`.

A temporary probe on the old code, removed since, expanded the UTC-calendar document to 08:00 UTC on all four dates (10:00 in Paris from 29 March), the New York rendering from 03:00 UTC, and the series with its override to five occurrences, the moved one at 09:00 UTC beside the original at 08:00 UTC on 29 March.

Two tests pinned the display-zone premise and failed after the fix, on the defect itself: `a_utc_stamp_keeps_its_instant_when_google_names_a_display_zone` asserted `DTSTART:20260814T090000Z` and no TZID, and `an_update_carries_the_display_zone_over_so_a_series_does_not_drift` the string `2026-10-20T07:00:00Z` where the merged start is now `2026-10-20T09:00:00` in Europe/Paris, the same instant. They became `a_utc_stamp_keeps_its_instant_in_the_zone_google_names` (TZID and wall time, the instant read back unchanged) and `an_update_keeps_the_zone_so_a_series_does_not_drift` (zone and instant kept through projection, read and merge; a document holding UTC stamps still takes the server's zone).

Run through `nix develop --command`: `cargo fmt`; `cargo test --features ical` green, 53 tests in the iCalendar module, the 9 live tests of tests/google.rs ignored and not run; `cargo test --all-features` 212 passed, 9 ignored; `cargo clippy --all-features --all-targets -- -D warnings` clean; `cargo build --no-default-features` and `cargo build --no-default-features --features ical` built. No live call was made.

Left:

- Live check: E7, or a new weekly event at 09:00 in Paris in a calendar in UTC, read after the bump, expecting `DTSTART;TZID=Europe/Paris:20270315T090000`; and one event in a second zone in a calendar neither in UTC nor in the event's zone (GCAL-03 scenario 2, never observed live).
- calendula: its cairn/spec/projection.md "An instant outranks a display zone" states the old premise and contradicts this; rewrite it when calendula moves to the release carrying the fix. Its "Boundaries carry a zone or nothing" holds as written.
- calendula, neverest and the Android app get the fix with the next io-gcal release; documents their stores projected before keep UTC stamps until read again, `carry_zone` keeping the server's zone on their writes meanwhile.
- The io-family audit ledger (.github cairn changes/io-family-audit-2026-11-13, tasks.md GCAL-03) to tick.
- The release, to the owner.

Capabilities moved: events-ical ("A boundary is written in the zone Google names").
