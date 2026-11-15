---
cairn: change
id: boundaries-in-the-event-zone
status: landed
created: 2026-11-15
---

# Boundaries in the event's zone

## Why

The live testing campaign of 2026-11-15 (.github cairn changes/live-testing-2026-11-15, reports/calendula/gcal-google.md G-F2 and O1) created a weekly event with `DTSTART;TZID=Europe/Paris:20270315T090000` in a calendar whose own time zone is UTC, the zone Google gives a calendar created through the API. Google returns its start as `{"dateTime":"2027-03-15T08:00:00Z","timeZone":"Europe/Paris"}` and expands the series in Paris (`events.instances` lists `2027-03-29T07:00:00Z`, 09:00 CEST after the change of 28 March), but `GcalEvent::to_ical` projects `DTSTART:20270315T080000Z` with no TZID, so a client expanding the projected RRULE lists 2027-03-29 and 2027-04-05 at 10:00 in Paris instead of 09:00.

The cause is `push_boundary` (src/v3/rest/events/ical.rs:1305 before the change). With a `timeZone` naming a zone other than UTC, only a `dateTime` not stamped `Z` took the TZID path, and it took the literal digits, `ical_local` dropping the offset; a `Z` stamp fell through to `ical_utc` ("the instant wins"). Both branches rest on the premise that `timeZone` is the calendar's display zone and that Google renders a zoned boundary in that zone's own offset (calendula's log 2026-08-09-gcal-backend, its requirement "An instant outranks a display zone"). The API reference and the live answer say otherwise: `timeZone` is the event's own zone, the one a recurrence is expanded in, and `dateTime` is rendered in the zone of the response, the calendar's unless the request names another.

The io-family audit of 2026-11-13 (GCAL-03, .github cairn changes/io-family-audit-2026-11-13) named the second branch too: a `dateTime` in the offset of a third zone is read as wall time in the named one. From a calendar in New York the same start reads `2027-03-15T04:00:00-04:00` and projects `DTSTART;TZID=Europe/Paris:20270315T040000`, five hours early, and writing that document back would move the event on the server (by reading: the offset-less wall time it reads back takes no zone from the server copy).

## What

Under `ical`, in src/v3/rest/events/ical.rs, `push_boundary` writes a timed boundary whose `timeZone` names a zone other than UTC as the wall time of that zone, with its TZID, whatever offset its `dateTime` carries: an offset, `Z` included, makes it an instant, placed on the zone's clock through the bundled database (jiff's, the one `instant` and the VTIMEZONE synthesis already read); no offset makes it that wall time already. DTSTART, DTEND and the RECURRENCE-ID of an `originalStartTime` all go through it.

What stays:

- a `dateTime` already in the zone's own offset projects as before;
- a `timeZone` of UTC, or none, projects the instant in UTC; an all-day `date` projects as a DATE;
- a zone the database does not know keeps the instant in UTC, as a `Z` stamp did before: its wall time cannot be computed, and a TZID no VTIMEZONE defines would break RFC 5545 3.2.19. A `dateTime` without offset under such a zone keeps its TZID, the wall time being given (a document naming its own zone stashes its definition);
- EXDATE and RDATE are recurrence lines Google stores and returns as written, spliced back verbatim: no boundary path, nothing to convert;
- `GcalEvent::merge` keeps carrying the server's `timeZone` over to a UTC-stamped boundary of the document (`carry_display_zone`, renamed `carry_zone`): the documents projected before this change, which stores still hold, carry a zoned series as UTC stamps, and writing them back without the zone would move Google's expansion to UTC.

`GcalEvent::replaces` compares original starts as instants and `instance_window` reads the date of the original start with a day of margin on each side, so both read the zoned RECURRENCE-ID a series document now carries as they read the UTC one. `GcalEventSeries` reads documents and needs no change; a test pins that an override projected from a calendar in UTC replaces the third occurrence as a client expands it, and reads back as an instance that `replaces` the listed one.

## Scope / non-goals

- No live run: the fix is checked offline, against the shapes the live report and the API reference give. The audit's second scenario (a calendar in a zone other than UTC and other than the event's) was not observed live.
- calendula's projection.md "An instant outranks a display zone" states the old premise; it is calendula's to rewrite when it moves to the release carrying this fix (not this repository).
- No release: left to the owner.
