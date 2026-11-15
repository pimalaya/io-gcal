# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.1] - 2026-11-15

### Added

- Added `GcalEventSeries` (requires `ical` feature), a series document read as its master and its instances: `from_ical` gives the master `GcalEvent::from_ical` reads, its EXDATEs among its recurrence lines, each override read as a document holding it alone, and `left_out`, what Google cannot hold.

  `GcalEvent::from_ical` reads the master alone, so a create or an import of a series dropped its overrides with no word. `GcalEvent::replaces` tells the listed instance an override replaces, by its original start as a date or an instant, and `GcalEvent::instance_window` gives the `timeMin` and `timeMax` of an `events.instances` listing holding it. `left_out` names an override with a RANGE, one whose RECURRENCE-ID does not read, one beside an event that is no series, and the units a full stash goes without, which fail no series.

### Fixed

- Fixed `GcalEvent::to_ical` and `to_ical_series` writing a zoned boundary in the zone of its calendar rather than its own (requires `ical` feature).

  Google renders `dateTime` in the calendar's zone and names the event's own zone in `timeZone`, so a weekly event at 09:00 in Paris read from a calendar in UTC projected as `DTSTART:20270315T080000Z`, and a client expanded it at 10:00 in Paris after the daylight-saving change, where Google keeps 09:00. A `dateTime` in a third zone's offset was read as wall time in the named zone, hours off. A timed DTSTART, DTEND or RECURRENCE-ID whose `timeZone` names a zone other than UTC is now its wall time in that zone with its TZID (`DTSTART;TZID=Europe/Paris:20270315T090000`), whatever the offset; a zone the bundled database does not know keeps the instant in UTC.

## [0.5.0] - 2026-11-15

### Changed

- **BREAKING**: Changed the pimalaya-stream dependency to 0.4.

  Its `tls::Tls` moved the trusted certificate from `cert` to `server`, and gained `client`, the certificate presented when the server asks for one (mutual TLS).

- **BREAKING**: Changed the io-http dependency to 0.8.

## [0.4.0] - 2026-11-13

### Added

- Added `gcal_api_url`, building a Calendar API URL from its path segments, each percent-encoded on its own.
- Added `GcalEventIcalError::StashFull`, returned by `GcalEvent::from_ical` when a document holds more unmanaged lines than Google stores beside an event (requires `ical` feature). **Breaking** for an exhaustive match.

  It carries the event, stashing every line it has room for, and the lines left out.
- Added `GcalApiError`, the Calendar error envelope: its status and message, the reasons of `errors[]` and `details[]`, the canonical status, the delay the server asks for (`Retry-After`, else `retryDelay`) and whether the connection can be reused.
- Added `GcalSendError::api`, `retry_after`, `is_rate_limited` and `is_not_found`.
- Added an `Unknown(String)` variant to the 18 enums read from the server, keeping a value this crate does not know as the API spells it. **Breaking**: these enums are no longer `Copy`, and an exhaustive match gains an arm.

  A full update writes the value back as it was read.

### Changed

- **BREAKING**: Changed the io-http dependency to 0.7.

  `GcalSendError::Send` carries its `Http11SendError`, which gains `UnsupportedTransferCoding`, `TooManyInterimResponses` and `InvalidRequest`, and `HttpAuthBearer` no longer implements `Display`.
- Changed `GcalEvent::to_ical` and `to_ical_series` to fold every line they write at 75 octets, a recurrence line and a stashed line included (requires `ical` feature).

  The VTIMEZONE of a series come in name order, where those an exception alone names used to follow the master's.
- Changed `GcalSend::with_guard` and `GcalSend::delete_if_match` to return a `Result`, refusing an entity tag that holds a control character. **Breaking.**
- Changed every coroutine to percent-encode the ids of its request path, so an id is passed as the API returns it. **Breaking** for a caller that encoded an id itself.

  An empty, `.` or `..` id, or one holding a tab or a line break, is refused with `GcalSendError::InvalidRequest`.
- Changed `GcalSendError::Api` to carry a `GcalApiError`. **Breaking.**
- Changed `GcalSendError::is_retryable` to count a rate limit Calendar answers with a 403 (`rateLimitExceeded`, `userRateLimitExceeded`, `quotaExceeded`).
- Changed `GcalSendError::is_sync_token_expired` to be false for a 410 whose reason is `deleted`, which speaks of a resource that is gone and not of the token.

  Every other 410 still reads as an expired token, one giving an unknown reason or none included.
- Changed `GcalEvent::merge` to take from the server copy a status, transparency, visibility, attendee response or reminder method this crate does not know, which no document can hold (requires `ical` feature).

### Fixed

- Fixed an id holding `#`, `?` or `/` naming another resource than its own, the events of a subscribed calendar such as `en.usa#holiday@group.v.calendar.google.com` among them.
- Fixed `GcalEvent::from_ical` leaving an unmanaged line wider than 1024 octets out of the stash without a word (requires `ical` feature).

  A line of any width now runs over as many extended properties as it takes.
- Fixed `GcalEvent::from_ical` overflowing on a VALARM whose trigger counts more seconds than fit, which panicked a build with overflow checks (requires `ical` feature).

  Such an alarm stays in the stash.
- Fixed one value outside an enum failing a whole response, a page of events or the calendar list among them.
- Fixed `GcalEvent::merge` turning the default reminders of the calendar back on for an event whose reminders are off (requires `ical` feature).

  A document without VALARM leaves `useDefault` as the server has it, and an alarm taken out of a document turns the reminders off.
- Fixed `GcalEvent::from_ical` stashing a folded line with its folds, which `to_ical` gave back with a stray CR in its value at each fold (requires `ical` feature).

  The stash now holds unfolded lines, a stash stored folded reads back whole, and a folded RRULE, EXRULE, RDATE or EXDATE is sent unfolded.
- Fixed `GcalEvent::to_ical` and `to_ical_series` splicing the recurrence, the stash, the alarms and the exceptions into a UID, SUMMARY, DESCRIPTION, LOCATION or name holding `END:VEVENT` or `END:VCALENDAR` (requires `ical` feature).

  The document is built as one tree and serialized once.

### Security

- Fixed the `Debug` of `GcalChannel` and of `GcalEntryPoint`, hence the trace logs, showing the token of a channel and the PIN, access code, meeting code, passcode and password of a conference entry point.
- Fixed a bearer token, a content type or a method holding a line break adding lines to the request.

  The first resume fails with `GcalSendError::InvalidRequest`, before any write.

## [0.3.1] - 2026-11-12

### Fixed

- `GcalEvent::from_ical` gives a VEVENT with neither DTEND nor DURATION the end RFC 5545 3.6.1 implies (the start itself after a DATE-TIME start, the next day after a DATE one) instead of refusing it with `NoEnd`; Google accepts a zero-length event. `NoEnd` now means an end the document gives but Google cannot read (a floating DTEND, a negative DURATION, hours after a DATE start).
- `GcalEvent::to_ical` writes no DTEND equal to a timed DTSTART, which RFC 5545 3.8.2.2 forbids, the event ending at its start without one; a stashed DTEND or DURATION at the VEVENT's own level no longer comes back beside the managed end.

## [0.3.0] - 2026-11-04

### Changed

- `GcalEvent::from_ical` accepts a VEVENT giving its end as a DURATION (RFC 5545 3.8.2.5) instead of refusing it with `NoEnd`: the end is computed from the DTSTART, in its zone and its shape, and the DURATION is dropped, so the event reads back with a DTEND. A negative DURATION, or one with hours after a `VALUE=DATE` start, is still refused.
- Bumped ical-rs to 0.7, whose `IcalParseError` `GcalEventIcalError::Parse` carries; it gains `TooDeep`, refusing a component nested deeper than 16. **Breaking.**
- Bumped io-http to 0.6, whose `Http11SendError` `GcalSendError::Send` carries; it gains `HeadersTooLarge` and `BodyTooLarge`, refusing a response over 64 KiB of headers or 256 MiB of body. io-http now encodes base64 through base64-codec. **Breaking.**
- Raised the minimum Rust version to 1.89.

## [0.2.0] - 2026-10-09

### Added

- `GcalEvent::from_ical` reads `X-PIMDIR-ONLINE-MEETING:TRUE` (pimdir STORAGE Annex B.1) as a request for a Google Meet: a `conferenceData.createRequest` whose id is drawn from the UID and DTSTAMP, so pushing the same document again replays the meeting instead of creating another. Google honours it on a write carrying `conferenceDataVersion=1`. The property is never stashed.

### Changed

- Bumped ical-rs to 0.6, whose `IcalParseError` `GcalEventIcalError::Parse` carries. Lines built from Google are folded at 75 octets. **Breaking.**
- The ways of joining a conference are minted as standard `CONFERENCE` properties (RFC 7986 5.11), one per entry point, with `VALUE=URI`, its `FEATURE` (`AUDIO,VIDEO` for video, `PHONE`, `AUDIO` for SIP) and its `LABEL`, instead of `X-GOOGLE-CONFERENCE`. Both are dropped by `from_ical`, the server value staying authoritative.
- `GcalEvent::merge` keeps the conference the server holds, and a create request still pending, so a write with `conferenceDataVersion=1` never drops them; a requested meeting is only asked for when the event has none.
- One occurrence of a series is written through its instance: `GcalEvent::from_ical` reads a document holding it alone, its RECURRENCE-ID as `originalStartTime` instead of a stashed line, and `merge` keeps the `recurringEventId` and `originalStartTime` of the server copy. `to_ical` writes the RECURRENCE-ID of any event carrying an `originalStartTime`, so such a document round-trips.

### Fixed

- `GcalEvent::from_ical` read the first VEVENT of a document, an override when it came before its series: it now reads the series master, the VEVENT without RECURRENCE-ID.

## [0.1.3] - 2026-10-01

### Changed

- The VTIMEZONE synthesis of the `ical` feature now comes from ical-rs's `tzdb` feature, where it moved so the Microsoft Graph projection shares it. Nothing changes in what is synthesized.

## [0.1.2] - 2026-10-01

### Added

- Added the `ical` feature: `GcalEvent::to_ical`, `to_ical_series` and `from_ical` project an event, or a recurring series with its exceptions, onto an iCalendar document and back, and `merge` carries the provider-only fields of the server copy over.

  Provider-scoped fields ride as read-only `X-GOOGLE-*` properties, every other line round-trips through `extendedProperties.private`, and every TZID gets a VTIMEZONE synthesized from the bundled time zone database. The projection moved here from Calendula, stash keys and `PRODID` included, so events Calendula already stashed still read back.

## [0.1.1] - 2026-09-28

### Fixed

- Fixed `no_std` builds pulling in `std` ([io-gmail#2]).

  The `serde_variant` dependency was dropped, enum query parameters now go through the in-crate query serializer.

## [0.1.0] - 2026-08-15

### Added

- Added the I/O-free coroutines for the whole Google Calendar API v3.
- Added `GcalClientStd`, a std blocking client behind the `client` feature.
- Added an optional `If-Match` guard to event update, patch and delete.

  `GcalSendError::is_precondition_failed` recognises the 412 a stale tag returns.

[unreleased]: https://github.com/pimalaya/io-gcal/compare/v0.5.1..HEAD
[0.5.1]: https://github.com/pimalaya/io-gcal/compare/v0.5.0..v0.5.1
[0.5.0]: https://github.com/pimalaya/io-gcal/compare/v0.4.0..v0.5.0
[0.4.0]: https://github.com/pimalaya/io-gcal/compare/v0.3.1..v0.4.0
[0.3.1]: https://github.com/pimalaya/io-gcal/compare/v0.3.0..v0.3.1
[0.3.0]: https://github.com/pimalaya/io-gcal/compare/v0.2.0..v0.3.0
[0.2.0]: https://github.com/pimalaya/io-gcal/compare/v0.1.3..v0.2.0
[0.1.3]: https://github.com/pimalaya/io-gcal/compare/v0.1.2..v0.1.3
[0.1.2]: https://github.com/pimalaya/io-gcal/compare/v0.1.1..v0.1.2
[0.1.1]: https://github.com/pimalaya/io-gcal/compare/v0.1.0..v0.1.1
[0.1.0]: https://github.com/pimalaya/io-gcal/compare/root..v0.1.0

[io-gmail#2]: https://github.com/pimalaya/io-gmail/issues/2
