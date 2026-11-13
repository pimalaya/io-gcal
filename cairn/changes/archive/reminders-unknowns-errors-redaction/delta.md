---
cairn: delta
change: reminders-unknowns-errors-redaction
---

# Delta

## ADDED Requirements

### Requirement: A document without alarm leaves the reminders as the server has them (events-ical)
`GcalEvent::to_ical` SHALL write a VALARM for each reminder override of an event that is off the defaults of its calendar, and none otherwise: an event on the defaults and an event with its reminders turned off both project to no VALARM, and no property tells them apart. `GcalEvent::merge` SHALL therefore read a document holding no VALARM Google models as saying nothing of `useDefault`, and keep the one of the server copy: the defaults stay on, reminders turned off stay off, and overrides the document no longer holds are removed without the defaults coming back. A document holding such a VALARM SHALL set the overrides whatever the server copy says. `GcalEvent::from_ical` alone, with no server copy to merge onto, SHALL put an event without VALARM on the defaults of its calendar.

### Requirement: A value this crate does not know comes from the server copy (events-ical)
A status, a transparency, a visibility, the response of an attendee or the method of a reminder that reads as `Unknown` SHALL project to no line, parameter or VALARM. `GcalEvent::merge` SHALL take it from the server copy when the document gives none in its place: the status, transparency and visibility of the event, the response of the attendee of the same address, and every reminder override whose method is unknown.

### Requirement: A refusal keeps what the server said (requests)
A non-2xx answer SHALL come back as `GcalSendError::Api` carrying a `GcalApiError`: the status (`error.code`, else the HTTP status), the message (else the raw body), the reasons of `error.errors[]` in order, the canonical `error.status`, the reasons of `error.details[]`, the delay the server asks for (the `Retry-After` header in seconds, else the `retryDelay` of `error.details[]`, a date or a malformed value reading as none) and whether the connection can be reused. A part of the envelope in an unexpected shape SHALL NOT cost the rest.

### Requirement: A caller tells the refusals apart without reading the message (requests)
`is_rate_limited` SHALL be true for a 429, a `RESOURCE_EXHAUSTED` status, and a 403 whose reason is `rateLimitExceeded`, `userRateLimitExceeded`, `quotaExceeded` or `RATE_LIMIT_EXCEEDED`, and false for `dailyLimitExceeded`, which waiting does not lift. `is_retryable` SHALL be true for a rate limit and for a 500, 502, 503 or 504. `is_sync_token_expired` SHALL be true for every 410 but the one whose reason is `deleted`, which speaks of a resource that is gone and not of the token: `fullSyncRequired`, `updatedMinTooLongAgo`, a reason this crate does not know and no reason at all SHALL all read as an expired token, since a new baseline loses nothing and a missed expiry stalls the sync for good. `is_not_found` SHALL be true for a 404 and `is_precondition_failed` for a 412.

### Requirement: A value outside an enum is kept (resources)
Every enum read from the server SHALL carry an `Unknown(String)` variant, which a value the reference does not list reads as, spelled as the API sent it, and which is written back as that same string. One such value SHALL NOT fail the response that holds it. The enums only ever sent, `GcalSendUpdates` and `GcalEventsOrderBy`, stay closed.

### Requirement: A secret stays out of the debug form (resources)
The `Debug` of `GcalChannel` SHALL show `[REDACTED]` in place of its token, and the `Debug` of `GcalEntryPoint` in place of its PIN, access code, meeting code, passcode and password, an absent one still showing as absent. The trace logs, which print resources through `Debug`, SHALL therefore carry none of them.

## MODIFIED Requirements

### The sync token loop (sync)
A token eventually expires, and the server answers HTTP 410 rather than degrading silently. `GcalSendError::is_sync_token_expired` recognises it: every 410 is one, whatever reason it gives or none, but the 410 whose reason is `deleted`, which speaks of an event that is gone and not of the token.

## REMOVED Requirements

None.
