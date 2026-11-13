---
cairn: change
id: whole-stash-and-encoded-ids
status: landed
created: 2026-11-13
---

# A stash that drops nothing unsaid, ids that stay in their segment

## Why

Four defects, the first left open by the change ical-stash-and-tree, the others found by the family audit of 2026-11-13 (GCAL-01, high; GCAL-09 and GCAL-07, low).

- The stash left out, without a word, every logical line wider than 1024 octets: the line stayed in the local document and was gone at the next read from the server. Since ical-stash-and-tree the width is that of the logical line, so a wide line inside a stashed component, which used to pass as folded pieces, was left out too.
- Every coroutine pasted its ids into the request path with `format!` and `Url::join`, at 29 sites. An id holding `#` lost everything after it (the ids of the subscribed holiday, birthday and contacts calendars hold one, so their events never listed: the request became a `calendars.get` on `en.usa`), one holding `?` added query parameters, and `../..` climbed out of its collection (`events.delete` became `calendars.delete`).
- An entity tag was written into the `If-Match` header as given, so a tag holding CR LF added headers, or a second request, to an authorized exchange.
- `lead_minutes` multiplied and added the seconds of a VALARM trigger unchecked: `TRIGGER:-P99999999999999999W` panicked a build with overflow checks, and wrapped into an arbitrary reminder without them.

## What

- The stash stores a line whatever its width. It already cut its text into extended properties between any two characters, so a wide line only had to stop being filtered: it runs over as many properties as it takes and the reader, which joins them before splitting on the line feed, reads it back whole. No fold is added.
- The two stashes share a budget of 30,000 octets, under the 32 kB the API documents for the private extended properties of an event. A unit (one property line, or one component whole) that does not fit is left out and `GcalEvent::from_ical` returns `GcalEventIcalError::StashFull { event, left_out }`: the event with everything that fits, and the lines left out. A caller that propagates the error loses nothing unsaid; one that wants the write anyway takes the event out of the error, knowing what it goes without.
- `gcal_api_url` builds every request URL from its path segments through `path_segments_mut`, and refuses a segment that is empty, `.` or `..`, or that holds a tab or a line break, which the URL would drop or strip. The 38 coroutines use it, the 9 with a fixed path too, so no `join` is left to copy.
- `GcalSend::with_guard` refuses an entity tag holding a control character, and returns a `Result`, as `delete_if_match` now does.
- `lead_minutes` uses checked arithmetic and answers `None` on overflow, so the alarm stays in the stash.

Breaks: `GcalSend::with_guard` and `GcalSend::delete_if_match` return `Result<Self, GcalSendError>`; `GcalEventIcalError` gains the `StashFull` variant; an id a caller percent-encoded itself is now encoded twice.
