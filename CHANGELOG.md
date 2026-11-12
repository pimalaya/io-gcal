# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[unreleased]: https://github.com/pimalaya/io-gcal/compare/v0.3.1..HEAD
[0.3.1]: https://github.com/pimalaya/io-gcal/compare/v0.3.0..v0.3.1
[0.3.0]: https://github.com/pimalaya/io-gcal/compare/v0.2.0..v0.3.0
[0.2.0]: https://github.com/pimalaya/io-gcal/compare/v0.1.3..v0.2.0
[0.1.3]: https://github.com/pimalaya/io-gcal/compare/v0.1.2..v0.1.3
[0.1.2]: https://github.com/pimalaya/io-gcal/compare/v0.1.1..v0.1.2
[0.1.1]: https://github.com/pimalaya/io-gcal/compare/v0.1.0..v0.1.1
[0.1.0]: https://github.com/pimalaya/io-gcal/compare/root..v0.1.0

[io-gmail#2]: https://github.com/pimalaya/io-gmail/issues/2
