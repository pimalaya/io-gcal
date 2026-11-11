---
cairn: log
change: events-without-end
date: 2026-11-11
---

# Events without an end

Found on the Pimalaya Android app while recording its Google verification video (android docs/providers.md, 2026-11-11): a timed event created with a start and no end failed its push with `NoEnd`. Google accepts a zero-length event (checked live by the owner the same day).

- `GcalEvent::from_ical`: a VEVENT with neither DTEND nor DURATION ends where RFC 5545 3.6.1 says, the start itself after a DATE-TIME, the next day after a DATE, through the same path a DURATION takes, so the end keeps the start's zone and shape. `NoEnd` stays for an end the document gives but Google cannot read; its message says so.
- `GcalEvent::to_ical`: no DTEND equal to a timed DTSTART (RFC 5545 3.8.2.2 wants it later), compared as instants when both carry an offset; a DATE end is always written. A DTEND or DURATION at the VEVENT's own level is dropped from the stash before it is spliced back, a nested VALARM keeping its own DURATION. A zero-length event now round-trips with neither line.
- Tests failing first: `a_timed_event_without_end_ends_at_its_start`, `an_all_day_event_without_end_lasts_one_day` (both `NoEnd` before), `a_zero_length_event_projects_without_dtend`, `a_stashed_end_never_comes_back` (both writing a DTEND before); `a_given_end_is_kept_and_an_unreadable_one_refused` guards the remaining refusal.
- Checked: `cargo test --features ical` (lib 36 passed, every integration suite passing, the live Google suite ignored), clippy, both no_std builds, `cargo fmt`.
- Not done: io-msgraph refuses the same VEVENT (`MsgraphEventIcalError::NoEnd`, src/v1/rest/users/events/ical.rs); unreleased, the app picks it up with the next io-gcal release.

Capabilities moved: events-ical (added, first written for this change: Every event read has an end, No end written equal to a timed start).
