---
cairn: log
change: whole-stash-and-encoded-ids
landed: 2026-11-13
---

# A stash that drops nothing unsaid, ids that stay in their segment

Follows ical-stash-and-tree, and closes three findings of the family audit of 2026-11-13 (GCAL-01, high; GCAL-09 and GCAL-07, low).

## The stash

`chunk_into` filtered out every line wider than `MAX_STASH_CHUNK` before joining the others, so a wide unmanaged line never reached Google and was lost at the next read. The filter is gone. Nothing else was needed for the width: the function already cuts the joined text between any two characters, and `stashed` joins the chunks before it splits on the line feed, so a line of 3,000 octets rides three extended properties and reads back whole, in an older build too. The writer therefore adds no fold of its own.

What can still not be stored is a total: the API documents 32 kB of private extended properties per event, keys included. The two stashes now share `MAX_STASH_TOTAL`, 30,000 octets of lines with their line feeds, which leaves room for the keys of at most 31 chunks and about a kilobyte for the properties of other clients that `merge` keeps. `from_ical` gathers the remainder as units (a property line, or the lines of one component) and `chunk_into` stores each unit whole or hands it back, first fit, the VEVENT's units before the calendar's. When any is handed back, `from_ical` returns the new `GcalEventIcalError::StashFull { event, left_out }`: the event as it would have been returned, stashing all that fits, and the units left out as their logical lines; its message names each property and component. So the default path (`?`) refuses rather than loses, and a caller that prefers the write takes the event out of the error with the list in hand. The 32 kB figure is the documented one and was not checked live, nor was how Google counts it.

## The ids

Every coroutine built its URL as `Url::parse(GCAL_API_BASE)?.join(&format!("calendars/{calendar_id}/..."))`. `gcal_api_url(&["calendars", calendar_id, ...])`, in src/v3/send.rs, replaces the 38 sites, the 9 with a fixed path included: it extends the path through `path_segments_mut`, which percent-encodes each segment, after refusing with `InvalidRequest` a segment that is empty, `.` or `..` (the first leaves an empty segment, the other two are skipped by the URL) or that holds a tab or a line break (the URL strips them: `a\tb` went out as `ab`). An id is now passed as the API returns it: a caller that encoded `#` as `%23` itself, as the Android app does, sends `%2523` until it stops.

## The guard and the alarm

`GcalSend::with_guard` refuses an entity tag holding a control character and returns a `Result`, and `delete_if_match` with it; the private `build` holds the former body. `lead_minutes` multiplies and adds with `checked_mul` and `checked_add`.

## Tests

All failing on the former code: `a_line_wider_than_a_chunk_round_trips_whole` (both lines missing from the document), `a_stash_over_the_budget_tells_which_lines_stay_out` (written first as `from_ical(..).is_err()`, which failed, then completed), `a_lead_time_past_what_fits_leaves_the_alarm_in_the_stash` (`attempt to multiply with overflow`), and in the integration suites `writes_every_id_as_one_encoded_segment` (the 29 id-taking requests, each with the id of a holiday calendar and with `../a/b?c#d %`; it showed `GET /calendar/v3/calendars/en.usa` where `.../en.usa%23holiday@group.v.calendar.google.com/acl` was expected), `refuses_an_id_that_names_no_segment` (`""` gave `GET /calendar/v3/calendars//acl`) in tests/paths.rs and `refuses_an_entity_tag_holding_a_control_character` (the update, the patch and the deletion were all built) in tests/events.rs. The round-trip sweep of ical-stash-and-tree now draws values up to 4,000 octets.

One existing test pinned the loss and was changed: `a_line_too_long_for_a_chunk_stays_local_while_the_rest_is_chunked` asserted the wide line absent from the stash; it is now `a_line_too_long_for_a_chunk_runs_over_the_next_ones` and asserts it present.

## Left open

- The bearer token, `content_type` and `method` of the generic builders are still written as given. Refusing them here turns `get`, `delete`, `post_empty` and `with_method` into `Result`; the check io-http is to make in its own request covers them at once.
- The consumers that encode ids themselves have to stop: the Android app is the known one.
- Who owns the stash, this crate or ical-rs, is still undecided.

Capabilities moved: events-ical (modified: The stash holds logical lines; added: A full stash names what it leaves out, An alarm Google cannot hold stays in the stash), requests (new file, by accretion: An id is one path segment, An entity tag cannot end its header line).
