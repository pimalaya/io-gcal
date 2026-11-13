---
cairn: spec
capability: requests
status: current
---

# Requests

Every coroutine builds one authorized HTTP request through `GcalSend`. This file holds what the crate guarantees about the path and the entity tag guard of that request; the rest of the transport is not written down yet.

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
