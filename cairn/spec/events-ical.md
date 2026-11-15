---
cairn: spec
capability: events-ical
status: current
---

# Events as iCalendar

The `ical` feature projects a Google event to a VEVENT (`GcalEvent::to_ical`) and reads one back (`GcalEvent::from_ical`). This file holds what the projection decides about an event's end, about the stash of the lines it does not manage, about how the document is built, about which alarms become reminders, about what a merge keeps of the server copy and about how a series document is cut into its master and its instances; the rest of the projection is not written down yet.

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
`GcalEvent::from_ical` SHALL stash a property it does not manage as its logical line: its name, its parameters and its raw value, with no fold, no soft break, no line ending and no CR, one line per line feed of the stash. It SHALL stash a component it does not manage as its logical lines, from its BEGIN to its END. A recurrence line (RRULE, EXRULE, RDATE, EXDATE) SHALL reach `recurrence` as its logical line too. A line SHALL be stashed whatever its width: the stash is cut into extended properties of at most 1024 octets between any two characters, so a line wider than one runs over the next ones and reads back whole.

#### Scenario: A folded line
- GIVEN a VEVENT whose `URL` runs 144 octets, folded after the seventieth
- WHEN it is read, then written back
- THEN the stash holds the 144 octets on one line, and the VEVENT holds the same URL, folded at 75 octets, with no CR in its value

#### Scenario: A line wider than an extended property
- GIVEN a VEVENT holding an unmanaged property of 3,000 octets, and a component it does not manage holding another
- WHEN it is read, then written back
- THEN no extended property is wider than 1024 octets, and both lines are in the document, whole

### Requirement: A full stash names what it leaves out
The two stashes of an event SHALL hold at most 30,000 octets of lines together, the line feed after each line counted, which keeps them under the 32 kB the API documents for the private extended properties of an event. A unit, one property line or one component from its BEGIN to its END, SHALL be stashed whole or not at all, the units of the VEVENT taken first, in document order, a unit that does not fit being skipped and the ones after it still tried. When a unit is left out, `GcalEvent::from_ical` SHALL return `GcalEventIcalError::StashFull`, carrying the event with every unit that fits and the units left out as their logical lines, and its message SHALL name them. It SHALL never leave a line out without saying so.

#### Scenario: More lines than the budget
- GIVEN a VEVENT holding 40 unmanaged properties of about 1,000 octets, then a component of 2,000 octets, then a short property
- WHEN it is read
- THEN the error carries the event stashing the first 29 properties and the short one, and names the 11 others and the component as left out

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

### Requirement: An alarm Google cannot hold stays in the stash
`GcalEvent::from_ical` SHALL read a VALARM as a reminder override only when its action is DISPLAY or EMAIL and its trigger is a negative duration of whole minutes, four weeks at most. Any other alarm SHALL be stashed whole, one whose trigger is too large to count in seconds included.

### Requirement: A document without alarm leaves the reminders as the server has them
`GcalEvent::to_ical` SHALL write a VALARM for each reminder override of an event that is off the defaults of its calendar, and none otherwise: an event on the defaults and an event with its reminders turned off both project to no VALARM, and no property tells them apart. `GcalEvent::merge` SHALL therefore read a document holding no VALARM Google models as saying nothing of `useDefault`, and keep the one of the server copy: the defaults stay on, reminders turned off stay off, and overrides the document no longer holds are removed without the defaults coming back. A document holding such a VALARM SHALL set the overrides whatever the server copy says. `GcalEvent::from_ical` alone, with no server copy to merge onto, SHALL put an event without VALARM on the defaults of its calendar.

#### Scenario: Reminders turned off
- GIVEN an event whose server copy says `useDefault: false` with no override
- WHEN it is projected, edited by a client that keeps only what RFC 5545 defines, read back and merged onto the server copy
- THEN the event written still says `useDefault: false` with no override

#### Scenario: The last alarm removed
- GIVEN an event with one popup override, whose document a client removes the VALARM from
- WHEN the document is read back and merged onto the server copy
- THEN the event written says `useDefault: false` with no override

### Requirement: A value this crate does not know comes from the server copy
A status, a transparency, a visibility, the response of an attendee or the method of a reminder that reads as `Unknown` SHALL project to no line, parameter or VALARM. `GcalEvent::merge` SHALL take it from the server copy when the document gives none in its place: the status, transparency and visibility of the event, the response of the attendee of the same address, and every reminder override whose method is unknown.

### Requirement: A series is written with its instances
`GcalEventSeries::from_ical` SHALL read a document as its master, the VEVENT `GcalEvent::from_ical` reads with the calendar remainder, its EXDATEs among its recurrence lines, and every other VEVENT as an instance, read as a document holding it alone, with no calendar remainder, its RECURRENCE-ID as its `originalStartTime`. A stash past the budget SHALL NOT fail the series: each event keeps the units that fit, and the units left out join the series' `left_out`.

#### Scenario: A series with one EXDATE and one override
- GIVEN a daily series of four from 2026-11-23 14:00 in Europe/Paris, an EXDATE on 2026-11-25 and an override moving 2026-11-24 to 16:00
- WHEN it is read as a series
- THEN the master carries the RRULE and the EXDATE as its recurrence, and one instance replaces the occurrence originally starting 2026-11-24 14:00 in Europe/Paris, starting at 16:00

### Requirement: An instance names the occurrence it replaces
An instance SHALL replace a listed one when both original starts are dates and the same date, or both are date-times naming the same instant, an offset-less time read in its zone. Its listing window SHALL run from the midnight UTC of the day before its original start's date to that of two days after.

### Requirement: What Google cannot hold of a series is named
`GcalEventSeries::from_ical` SHALL drop nothing silently: it SHALL name, as its logical lines, every override whose RECURRENCE-ID carries a RANGE or does not read, every override beside an event that is no series, and every unit a stash goes without.
