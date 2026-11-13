---
cairn: spec
capability: events-ical
status: current
---

# Events as iCalendar

The `ical` feature projects a Google event to a VEVENT (`GcalEvent::to_ical`) and reads one back (`GcalEvent::from_ical`). This file holds what the projection decides about an event's end, about the stash of the lines it does not manage and about how the document is built; the rest of the projection is not written down yet.

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

### Requirement: The stash holds logical lines
`GcalEvent::from_ical` SHALL stash a property it does not manage as its logical line: its name, its parameters and its raw value, with no fold, no soft break, no line ending and no CR, one line per line feed of the stash. It SHALL stash a component it does not manage as its logical lines, from its BEGIN to its END. A recurrence line (RRULE, EXRULE, RDATE, EXDATE) SHALL reach `recurrence` as its logical line too. A logical line longer than 1024 octets SHALL NOT be stashed, a line of a stashed component included.

#### Scenario: A folded line
- GIVEN a VEVENT whose `URL` runs 144 octets, folded after the seventieth
- WHEN it is read, then written back
- THEN the stash holds the 144 octets on one line, and the VEVENT holds the same URL, folded at 75 octets, with no CR in its value

### Requirement: A stash stored folded reads back whole
`GcalEvent::to_ical` SHALL read a stash stored in the folded shape of its source as the lines it folds: every CR SHALL be dropped, a piece opening on a space or a tab SHALL continue the piece before it, and an empty piece SHALL be skipped.

#### Scenario: A fold stored with its CR
- GIVEN an extended property holding `URL:aaaa\r\n bbbb`, or `URL:aaaa\r\r\n bbbb` after one more round trip
- WHEN the event is projected
- THEN the VEVENT holds `URL:aaaabbbb`

### Requirement: The document is built as a tree
`GcalEvent::to_ical` and `to_ical_series` SHALL place every line by its place in the tree, never by searching the text: the recurrence lines, the stashed lines, then the alarms inside their VEVENT, the calendar remainder after the VEVENT of the master, the VEVENT of each exception after it, and ahead of the first VEVENT, in name order, the VTIMEZONE of every zone a TZID parameter names and no TZID property defines. No text of the event SHALL move a line out of its component. Every line written SHALL be folded at 75 octets, a recurrence line and a stashed line included.

#### Scenario: A summary holding a marker
- GIVEN a recurring event with one attendee, summarised `END:VEVENT`
- WHEN it is projected, then read back
- THEN the VEVENT holds the SUMMARY, the ATTENDEE and the RRULE, and the event read back has the same recurrence and attendee

### Requirement: Stored lines stay balanced
A stored line that is no content line SHALL be left out of the document. Among the stashed lines of a component, an END closing no BEGIN SHALL be left out, and a BEGIN left open SHALL be closed.
