---
cairn: change
id: ical-stash-and-tree
status: landed
created: 2026-11-13
---

# The stash holds logical lines, the document is built as a tree

## Why

Two defects of the `ical` projection reach user data, both found by the family audit of 2026-11-13 (GCAL-12 and GCAL-02, high), and a third costs time on a listing (GCAL-11, low).

- The stash stored an unmanaged line as `Display` writes it, the folds it arrived with included, joined its lines with a line feed and split on the line feed alone when reading. A folded line came back as `...\r\r\n ...`: one CR entered its value at each fold, and one more every time the document went through the projection again. The same writer fed `recurrence`, so a folded RRULE or EXDATE went to Google with a fold inside the string.
- `to_ical` serialized the tree, then spliced the recurrence lines, the stash and the alarms before the first `END:VEVENT` of the text, the calendar remainder and the exceptions of a series before the first `END:VCALENDAR`. The search was a substring search, so a UID, SUMMARY, DESCRIPTION, LOCATION, CN or LABEL holding the marker took the splice: the RRULE landed inside the SUMMARY line and every line after it outside the VEVENT. The strict parser accepts that document, so a local edit of the event then wrote it back to Google without its recurrence and its attendees.
- Each splice shifted the tail of the document, the zones were read from a second copy of its unfolded lines, and `to_ical_series` projected each exception as a whole document, zones minted and thrown away, before minting them again over the series.

## What

- `raw_line` writes the logical line from its parts (name, parameters, raw value), and `raw_component` walks the component instead of splitting its text. A CR, which no content line holds, is dropped on the way in, so a document the former projection left a CR in heals when it is read again.
- `stashed` reads both shapes: it drops every CR and re-joins a piece opening on a space or a tab onto the piece before it, so a stash stored at Google with its folds reads whole without being rewritten.
- `to_ical_series` builds one tree: the VEVENT of the master, the calendar remainder, the VEVENT of each exception, then the VTIMEZONE of every zone the tree names without defining, and serializes it once. `to_ical` is the series without exception. The recurrence lines and the stashed lines are tokenised into the tree (`push_lines`), folded afresh at 75 octets, and kept balanced: a stored line that is no content line and an END closing nothing are left out, a BEGIN left open is closed.
- Nothing moves in the public API, and no dependency is added.

Who owns the stash, this crate or ical-rs, is a decision the maintainer has not taken: it is fixed in place, nothing moved.
