---
cairn: delta
change: whole-stash-and-encoded-ids
---

# Delta

## ADDED Requirements

### Requirement: A full stash names what it leaves out (events-ical)
The two stashes of an event SHALL hold at most 30,000 octets of lines together, the line feed after each line counted, which keeps them under the 32 kB the API documents for the private extended properties of an event. A unit, one property line or one component from its BEGIN to its END, SHALL be stashed whole or not at all, the units of the VEVENT taken first, in document order, a unit that does not fit being skipped and the ones after it still tried. When a unit is left out, `GcalEvent::from_ical` SHALL return `GcalEventIcalError::StashFull`, carrying the event with every unit that fits and the units left out as their logical lines, and its message SHALL name them. It SHALL never leave a line out without saying so.

### Requirement: An alarm Google cannot hold stays in the stash (events-ical)
`GcalEvent::from_ical` SHALL read a VALARM as a reminder override only when its action is DISPLAY or EMAIL and its trigger is a negative duration of whole minutes, four weeks at most. Any other alarm SHALL be stashed whole, one whose trigger is too large to count in seconds included.

### Requirement: An id is one path segment (requests)
Every request path SHALL be built by `gcal_api_url` from its segments, each percent-encoded on its own, so that a calendar id, an event id, a rule id or a setting id holding `/`, `?`, `#`, `%` or a space names the resource it is the id of and no other. An id SHALL be passed as the API returns it, never encoded beforehand. A segment that is empty, `.` or `..`, or that holds a tab or a line break, SHALL be refused with `GcalSendError::InvalidRequest` before anything is written: the URL would drop or rewrite it.

### Requirement: An entity tag cannot end its header line (requests)
`GcalSend::with_guard`, `delete_if_match`, `put_json_if_match` and `patch_json_if_match`, and the event coroutines built on them, SHALL refuse with `GcalSendError::InvalidRequest` an entity tag holding a control character, a CR or an LF among them, before anything is written.

## MODIFIED Requirements

### Requirement: The stash holds logical lines (events-ical)
`GcalEvent::from_ical` SHALL stash a property it does not manage as its logical line: its name, its parameters and its raw value, with no fold, no soft break, no line ending and no CR, one line per line feed of the stash. It SHALL stash a component it does not manage as its logical lines, from its BEGIN to its END. A recurrence line (RRULE, EXRULE, RDATE, EXDATE) SHALL reach `recurrence` as its logical line too. A line SHALL be stashed whatever its width: the stash is cut into extended properties of at most 1024 octets between any two characters, so a line wider than one runs over the next ones and reads back whole.

## REMOVED Requirements

None.
