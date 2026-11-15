---
cairn: delta
change: series-exceptions-written-through-instances
---

# Delta

## ADDED Requirements

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

## MODIFIED Requirements

None.

## REMOVED Requirements

None.
