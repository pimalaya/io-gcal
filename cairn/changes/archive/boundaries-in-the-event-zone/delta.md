---
cairn: delta
change: boundaries-in-the-event-zone
---

# Delta

## ADDED Requirements

### Requirement: A boundary is written in the zone Google names (events-ical)
`GcalEvent::to_ical` and `to_ical_series` SHALL write a timed boundary (the DTSTART, the DTEND, and the RECURRENCE-ID an `originalStartTime` becomes) whose `timeZone` names a zone other than UTC as the wall time of that zone, under a TZID naming it: a `dateTime` carrying an offset, `Z` included, SHALL be read as an instant and placed on that zone's clock whatever the offset Google rendered it in, and a `dateTime` without one SHALL be taken as that wall time. A zone the bundled time zone database does not know SHALL keep the instant of a `dateTime` carrying an offset, written in UTC, and the wall time of one without, under its TZID. A boundary whose `timeZone` is UTC or absent SHALL be written in UTC when its `dateTime` carries an offset, and an all-day boundary as a DATE. An EXDATE or an RDATE SHALL be written as Google returns it, among the recurrence lines. `GcalEvent::merge` SHALL give a UTC-stamped boundary of the document the `timeZone` the server copy names, and leave every other boundary as the document gives it.

#### Scenario: A series in Paris read from a calendar in UTC
- GIVEN a weekly event of four whose start Google returns as `{"dateTime":"2027-03-15T08:00:00Z","timeZone":"Europe/Paris"}`
- WHEN it is projected
- THEN the VEVENT holds `DTSTART;TZID=Europe/Paris:20270315T090000` and its RRULE, the document defines Europe/Paris, and the series expands to 2027-03-29 at 09:00 in Paris, 07:00 UTC, as Google's `events.instances` lists it

#### Scenario: The offset of a third zone
- GIVEN the same start read from a calendar in New York, `{"dateTime":"2027-03-15T04:00:00-04:00","timeZone":"Europe/Paris"}`
- WHEN it is projected
- THEN the VEVENT holds `DTSTART;TZID=Europe/Paris:20270315T090000`

#### Scenario: An override in the zone of its series
- GIVEN that series and its instance whose `originalStartTime` Google returns as `{"dateTime":"2027-03-29T07:00:00Z","timeZone":"Europe/Paris"}`, moved to 11:00
- WHEN both are projected as one series document
- THEN the override holds `RECURRENCE-ID;TZID=Europe/Paris:20270329T090000`, takes the place of the third occurrence as a client expands the series, and is read back by `GcalEventSeries::from_ical` as an instance that replaces the listed one

#### Scenario: A zone the database does not know
- GIVEN a start of `{"dateTime":"2027-03-15T10:00:00+02:00","timeZone":"Mars/Olympus_Mons"}`
- WHEN it is projected
- THEN the VEVENT holds `DTSTART:20270315T080000Z` and no TZID

## MODIFIED Requirements

None.

## REMOVED Requirements

None.
