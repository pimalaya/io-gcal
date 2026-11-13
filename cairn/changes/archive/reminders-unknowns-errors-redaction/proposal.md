---
cairn: change
id: reminders-unknowns-errors-redaction
status: landed
created: 2026-11-13
---

# Reminders kept off, unknown values kept, errors told apart, secrets kept out of logs

## Why

Four findings of the family audit of 2026-11-13 (GCAL-06, GCAL-05 and GCAL-04, medium; GCAL-10, low).

- An event whose reminders are turned off (`useDefault: false`, no override) projects to a VEVENT without VALARM, as an event on the defaults of its calendar does. `from_ical` reads a document without VALARM as `useDefault: true` and `merge` took the reminders from the projection, so editing the title of such an event turned the default notifications of the calendar back on for it.
- The 18 enums read from the server were closed: one value outside them failed the whole response with `ParseResponse`. `eventType` rides on every event and has grown several times, so a new value would stop a calendar from listing until a release, and one unknown `accessRole` would hide every calendar of the list.
- An API error kept the code and the message of the envelope and nothing else. A rate limit answered with a 403 could only be told from a permission refusal by its English message (the live suite of this crate does exactly that), `is_sync_token_expired` was true for any 410, a deleted event included, and the `Retry-After` header was dropped with every other.
- `GcalChannel` and `GcalEntryPoint` derived `Debug`, and every coroutine traces its input and its output through it: a trace log carried the token that authenticates a notification to its receiver, and the PIN and codes of every conference listed.

## What

- **Reminders.** `merge` reads a document holding no VALARM Google models as saying nothing of `useDefault` and keeps the one of the server copy. The other fix on offer, a marker property minted when the reminders are off and read on the way back, was not taken: it lives only as long as every client the document goes through keeps a property it does not know, and the first one that drops it brings the defect back, while what `merge` needs, the server copy, is always at hand when an event is written. The merge also settles a case the marker leaves alone: the last VALARM removed from a document now turns the reminders off, where the defaults used to come back.
- **Enums.** Each of the 18 gains `Unknown(String)`, an untagged variant: a value the reference does not list reads as it and is written back as the same string, so a full update never sends a placeholder nor resets the field. The cost is `Copy`, which the enums lose. `#[serde(other)]` was not taken: it keeps `Copy` but forgets the value, and `events.update` replaces the whole resource. In the projection such a value has no iCalendar form: it projects to nothing and `merge` takes it from the server copy.
- **Errors.** `GcalApiError`, the shape io-gtasks landed the same day, replaces the two fields of `GcalSendError::Api`: status, message, reasons, canonical status, detail reasons, the delay asked for and the connection flag. `is_sync_token_expired` stays true for a 410 except the one whose reason is `deleted`.
- **Debug.** Hand-written for the two types, `[REDACTED]` in place of each secret that is set.

Breaks: `GcalSendError::Api` is a tuple variant holding a `GcalApiError`; the 18 enums are no longer `Copy` and gain a variant; `is_sync_token_expired` is false for a 410 whose reason is `deleted`.
