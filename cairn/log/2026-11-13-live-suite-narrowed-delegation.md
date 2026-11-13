---
cairn: log
change: live-suite-narrowed-delegation
landed: 2026-11-13
---

# The live suite under the narrowed delegation

A test-only change: nothing under src/ moved, and no capability did.

Since 2026-11-07 the domain-wide delegation of the service account behind tests/google.rs grants, for Calendar, `calendar.events`, `calendar.calendars` and `calendar.calendarlist.readonly` only. The suite still asked the full `calendar` scope for the delegated user, and one refused scope fails the whole token request with `unauthorized_client`, so `delegated_account`, `delegated_calendar`, `insert_keeps_ical_uid` and `subscription` failed in CI from 2026-11-12. The service account acting as itself still gets the full scope.

The delegated principal now mints its token with those three scopes, and each flow takes the principal it runs as. What the live API answered on 2026-11-13, per call, under the three scopes:

- Granted: `colors.get`, `calendarList.list`, `calendarList.get`, `calendars.insert`, `get`, `patch`, `update` and `delete`, and every `events` call the suite makes (`insert` with guests and `sendUpdates`, `get`, `patch`, `update`, `import`, `instances`, `quickAdd`, `list` with and without a sync token, `move`, `delete`).
- Refused with 403 `insufficientPermissions`: `settings.list` and `settings.get` (they take `calendar.settings.readonly`), `calendarList.insert`, `update`, `patch` and `delete` (`calendar.calendarlist`), `acl.list` and `acl.insert` (`calendar.acls`), `freebusy.query` (`calendar.freebusy`).

So the settings, the calendar list entry patch, the ACL walk and the free/busy query run for the service account only, and the delegated passes run everything else.

`subscription` changed shape, since no principal of the run can subscribe any more: the delegated user cannot write its calendar list, and cannot share a calendar of its own with the service account either. The service account still shares a throwaway calendar with the delegated user, who now reads it and its one event through the share (a shared calendar is not added to the list of the user it is shared with: `calendarList.get` answers 404). The calendar list writes moved to the service account, on the calendar it owns: `calendarList.insert` of a calendar already listed answers the entry, `update` rewrites it, and `delete` is refused with 403 `cannotUnsubscribeFromOwnedCalendar`, which the test asserts. The reader side of a subscription (an entry inserted with the `reader` role, then removed) is no longer covered live; it comes back the day the delegation grants `calendar.calendarlist`.

Left for the owner: CONTRIBUTING.md still describes two live tests, a token-only setup and a key that needs no domain-wide delegation.
