# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[unreleased]: https://github.com/pimalaya/io-gcal/compare/v0.1.1..HEAD
[0.1.1]: https://github.com/pimalaya/io-gcal/compare/v0.1.0..v0.1.1
[0.1.0]: https://github.com/pimalaya/io-gcal/compare/root..v0.1.0

[io-gmail#2]: https://github.com/pimalaya/io-gmail/issues/2
