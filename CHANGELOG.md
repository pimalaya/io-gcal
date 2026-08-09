# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added the I/O-free coroutines covering the whole version 3 of the Google Calendar API: `acl` (list, get, insert, update, patch, delete, watch), `calendarList` (list, get, insert, update, patch, delete, watch), `calendars` (get, insert, update, patch, delete, clear, transferOwnership), `channels` (stop), `colors` (get), `events` (list, get, insert, update, patch, delete, import, instances, move, quickAdd, watch), `freebusy` (query) and `settings` (list, get, watch).
- Added the shared `GcalSend` request primitive over io-http, the `GcalCoroutine` contract and the `GcalClientStd` blocking client (`client` feature), with `connect` opening the TCP and TLS connection behind a TLS feature (`rustls-ring` default, `rustls-aws`, `native-tls`).

[unreleased]: https://github.com/pimalaya/io-gcal/compare/root..HEAD
