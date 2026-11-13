---
cairn: spec
capability: resources
status: current
---

# Resources

The types under `v3::rest` mirror the resources of the Calendar reference. This file holds what the crate guarantees about the values it does not know and about what a resource shows of itself in a log; the rest of the representation is not written down yet.

### Requirement: A value outside an enum is kept
Every enum read from the server SHALL carry an `Unknown(String)` variant, which a value the reference does not list reads as, spelled as the API sent it, and which is written back as that same string. One such value SHALL NOT fail the response that holds it. The enums are the status, transparency, visibility and type of an event, the response status and asynchronous operation of an attendee, the method of a reminder, the birthday type, the auto decline mode, the chat status, the working location type, the status code of a conference request, the entry point type, the conference solution type, the scope type and the access role of an ACL rule, the calendar notification type and the channel type. The enums only ever sent, `GcalSendUpdates` and `GcalEventsOrderBy`, stay closed.

#### Scenario: A page holding a new event type
- GIVEN a page of events, one of which says `"eventType": "somethingNew"`
- WHEN the page is listed, then that event written back by a full update
- THEN every event of the page is returned, and the body of the update says `"eventType": "somethingNew"`

### Requirement: A secret stays out of the debug form
The `Debug` of `GcalChannel` SHALL show `[REDACTED]` in place of its token, and the `Debug` of `GcalEntryPoint` in place of its PIN, access code, meeting code, passcode and password, an absent one still showing as absent. The trace logs, which print resources through `Debug`, SHALL therefore carry none of them.
