---
cairn: log
change: reminders-unknowns-errors-redaction
landed: 2026-11-13
---

# Reminders kept off, unknown values kept, errors told apart, secrets kept out of logs

Closes four findings of the family audit of 2026-11-13 (GCAL-06, GCAL-05 and GCAL-04, medium; GCAL-10, low).

## Reminders (GCAL-06)

`from_ical` sets `useDefault` to whether the document holds no VALARM, which is right for a new document and wrong for the round trip of an event whose reminders are off: both project to no VALARM. `merge` now keeps `useDefault` of the server copy when the projection holds no override. Chosen over a marker property because it survives any client: a marker is an `X-` property a client may drop, and dropping it brings the defect back, whereas the merge reads nothing but the absence of VALARM, which every client keeps. It also changes one more case on purpose: an event that had overrides and whose document lost its last VALARM is written with `useDefault: false` and no override, where the calendar's defaults used to come back. `from_ical` is unchanged, so a document inserted without a server copy still lands on the defaults.

## Unknown values (GCAL-05)

The 18 enums read from the server (13 in src/v3/rest/events.rs, 2 in acl.rs, 1 each in calendars.rs, calendar_list.rs and channels.rs) each gain `#[serde(untagged)] Unknown(String)`. The known spellings are tried first, so nothing changes for them; any other string reads as `Unknown` and is written back as the same string, which is what keeps a full update from sending a placeholder or resetting the field. They are no longer `Copy`. `GcalSendUpdates` and `GcalEventsOrderBy`, only ever sent, stay as they were. In the iCalendar projection an unknown status, transparency, visibility, attendee response or reminder method projects to nothing, and `merge` takes it from the server copy (the attendee by its address), as it does for the fields without an iCalendar slot.

## Errors (GCAL-04)

`GcalSendError::Api` holds a `GcalApiError`, the shape of io-gtasks: `status`, `message`, `reasons`, `google_status`, `detail_reasons`, `retry_after` and `keep_alive`, with `parse`, `has_reason`, `is_rate_limited`, `is_retryable`, `is_not_found`, and here `is_sync_token_expired`, true for every 410 but the one whose reason is `deleted`. The test fails safe on purpose: the reason names are remembered, not verified, and more than one 410 calls for a full sync (`fullSyncRequired`, `updatedMinTooLongAgo`), so a reason this crate does not know, or none, reads as an expired token. A new baseline loses nothing, while a consumer told false on an expiry never resyncs and stays stuck on the error; `deleted` is the one case where the 410 speaks of the resource and not of the token. `GcalSendError` gains `api`, `retry_after`, `is_rate_limited` and `is_not_found`. The reasons are those of Google's error guide for Calendar as remembered (`rateLimitExceeded`, `userRateLimitExceeded`, `quotaExceeded` with a 403 or a 429; `fullSyncRequired`, `updatedMinTooLongAgo`, `deleted` with a 410) and were not checked live.

## Debug (GCAL-10)

`GcalChannel` and `GcalEntryPoint` write their `Debug` by hand, `Some("[REDACTED]")` in place of a token, PIN, access code, meeting code, passcode or password that is set. The URI of an entry point is left readable: it is what the projection writes into the document.

## Tests

Failing on the former code: `a_document_without_alarm_leaves_the_reminders_as_the_server_has_them` (`useDefault: true` came back); one `keeps_..._it_does_not_know` test per enum, 18 in all, through `assert_keeps_unknown` of tests/common/mod.rs (``unknown variant `somethingNew` ``); `lists_a_page_holding_values_it_does_not_know` and `lists_the_calendars_past_an_access_role_it_does_not_know` (`ParseResponse`), the first also checking that a full update writes the values back; `flags_a_rate_limit_reason_as_retryable` (`is_retryable` false on a 403 `rateLimitExceeded`) and `tells_a_deleted_resource_from_an_expired_sync_token` (true on a 410 `deleted`; it also holds `fullSyncRequired`, `updatedMinTooLongAgo`, an unknown reason and no reason to true), both first written against the former API; `hides_the_token_of_a_channel_from_its_debug` and `hides_the_access_codes_of_an_entry_point_from_its_debug` (the secrets printed). Written with the code, since they name what did not exist: `a_value_this_crate_does_not_know_comes_from_the_server_copy`, and the tests of the delay, the reasons and the connection flag ported from io-gtasks.

Existing tests touched for the new shapes, none for its expectations: `flags_the_transient_statuses_as_retryable` builds a `GcalApiError`; two assertions compare a borrowed enum where they copied one.

## Left open

- An event on the defaults of its calendar still projects to no VALARM, so a local client does not ring for it: minting one needs the default reminders of the calendar at projection time.
- The integer fields share the failure mode the enums had.
- The live suite still reads the message of a 403 to spot a rate limit (tests/google.rs, `rate_limited`): `is_retryable` now covers it, and the helper can go at the next live run.

Capabilities moved: events-ical (added: A document without alarm leaves the reminders as the server has them, A value this crate does not know comes from the server copy), requests (added: A refusal keeps what the server said, A caller tells the refusals apart without reading the message), sync (modified: the sync token loop), resources (new file, by accretion: A value outside an enum is kept, A secret stays out of the debug form).
