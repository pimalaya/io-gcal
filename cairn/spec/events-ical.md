---
cairn: spec
capability: events-ical
status: current
---

# Events as iCalendar

The `ical` feature projects a Google event to a VEVENT (`GcalEvent::to_ical`) and reads one back (`GcalEvent::from_ical`). This file holds what the projection decides about an event's end; the rest of the projection is not written down yet.

### Requirement: Every event read has an end
`GcalEvent::from_ical` SHALL give Google an end for every VEVENT it accepts: its DTEND, else its DATE-TIME or DATE start shifted by its DURATION, else the end RFC 5545 3.6.1 implies, the start itself after a DATE-TIME start and the next day after a DATE one. The end SHALL keep the start's zone and shape. It SHALL refuse with `NoEnd` only an end the document gives and Google cannot read: a floating DTEND, a negative DURATION, a DURATION with a time part after a DATE start.

#### Scenario: A timed event with no end
- GIVEN a VEVENT with `DTSTART;TZID=Europe/Paris:20260814T090000` and neither DTEND nor DURATION
- WHEN it is read
- THEN Google receives an end of 2026-08-14T09:00:00 in Europe/Paris, a zero-length event

#### Scenario: An all-day event with no end
- GIVEN a VEVENT with `DTSTART;VALUE=DATE:20261231` and no end
- WHEN it is read
- THEN Google receives an end date of 2027-01-01

### Requirement: No end written equal to a timed start
`GcalEvent::to_ical` SHALL write no DTEND equal to a timed DTSTART, which RFC 5545 3.8.2.2 forbids, the event then ending at its start; it SHALL compare the two as instants when both carry an offset. It SHALL always write a DATE end. A DTEND or DURATION at the VEVENT's own level SHALL NOT come back from the stash beside the end it manages; a nested component keeps its own.

#### Scenario: A zero-length event round-trips
- GIVEN a VEVENT with a timed DTSTART and no end
- WHEN it is read, then written back
- THEN the VEVENT holds neither DTEND nor DURATION
