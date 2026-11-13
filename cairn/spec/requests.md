---
cairn: spec
capability: requests
status: current
---

# Requests

Every coroutine builds one authorized HTTP request through `GcalSend`. This file holds what the crate guarantees about the path, the entity tag guard and the head of that request, and about the error a refusal comes back as; the rest of the transport is not written down yet.

### Requirement: An id is one path segment
Every request path SHALL be built by `gcal_api_url` from its segments, each percent-encoded on its own, so that a calendar id, an event id, a rule id or a setting id holding `/`, `?`, `#`, `%` or a space names the resource it is the id of and no other. An id SHALL be passed as the API returns it, never encoded beforehand. A segment that is empty, `.` or `..`, or that holds a tab or a line break, SHALL be refused with `GcalSendError::InvalidRequest` before anything is written: the URL would drop or rewrite it.

#### Scenario: An id holding a fragment mark
- GIVEN the calendar id `en.usa#holiday@group.v.calendar.google.com`
- WHEN its events are listed
- THEN the request line is `GET /calendar/v3/calendars/en.usa%23holiday@group.v.calendar.google.com/events`

#### Scenario: An id climbing a level
- GIVEN the event id `..`
- WHEN the event is deleted
- THEN the coroutine is not built, and the error is `InvalidRequest`

### Requirement: An entity tag cannot end its header line
`GcalSend::with_guard`, `delete_if_match`, `put_json_if_match` and `patch_json_if_match`, and the event coroutines built on them, SHALL refuse with `GcalSendError::InvalidRequest` an entity tag holding a control character, a CR or an LF among them, before anything is written.

### Requirement: No value of the caller adds a line to the request
A bearer token or a content type holding a CR, an LF or a NUL, and a method that is not an HTTP token, SHALL fail the first resume of the coroutine with `GcalSendError::InvalidRequest`, before anything is written. The check is the one io-http makes when it serializes the request; its refusal SHALL NOT come back as `GcalSendError::Send`, and the reason of a refused header SHALL name the header and never its value.

#### Scenario: A token holding a line break
- GIVEN the bearer token `abc`, a CR LF, then `X-Injected: 1`
- WHEN a coroutine built with it is resumed
- THEN it completes with `InvalidRequest` naming `Authorization`, and asks for no write

### Requirement: A refusal keeps what the server said
A non-2xx answer SHALL come back as `GcalSendError::Api` carrying a `GcalApiError`: the status (`error.code`, else the HTTP status), the message (else the raw body), the reasons of `error.errors[]` in order, the canonical `error.status`, the reasons of `error.details[]`, the delay the server asks for (the `Retry-After` header in seconds, else the `retryDelay` of `error.details[]`, a date or a malformed value reading as none) and whether the connection can be reused. A part of the envelope in an unexpected shape SHALL NOT cost the rest.

### Requirement: A caller tells the refusals apart without reading the message
`is_rate_limited` SHALL be true for a 429, a `RESOURCE_EXHAUSTED` status, and a 403 whose reason is `rateLimitExceeded`, `userRateLimitExceeded`, `quotaExceeded` or `RATE_LIMIT_EXCEEDED`, and false for `dailyLimitExceeded`, which waiting does not lift. `is_retryable` SHALL be true for a rate limit and for a 500, 502, 503 or 504. `is_sync_token_expired` SHALL be true for every 410 but the one whose reason is `deleted`, which speaks of a resource that is gone and not of the token: `fullSyncRequired`, `updatedMinTooLongAgo`, a reason this crate does not know and no reason at all SHALL all read as an expired token, since a new baseline loses nothing and a missed expiry stalls the sync for good. `is_not_found` SHALL be true for a 404 and `is_precondition_failed` for a 412.

#### Scenario: A burst over the quota
- GIVEN a 403 whose `errors[0].reason` is `rateLimitExceeded`, with a `Retry-After: 120` header
- WHEN the exchange completes
- THEN the error is retryable and rate limited, and its delay is 120 seconds

#### Scenario: An event already deleted
- GIVEN a 410 whose reason is `deleted`
- WHEN the exchange completes
- THEN the error is not an expired sync token

#### Scenario: A 410 for a reason nobody listed
- GIVEN a 410 whose reason is `somethingNew`, or that gives none
- WHEN the exchange completes
- THEN the error is an expired sync token
