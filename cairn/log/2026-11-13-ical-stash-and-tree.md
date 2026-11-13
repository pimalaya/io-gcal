---
cairn: log
change: ical-stash-and-tree
landed: 2026-11-13
---

# The stash holds logical lines, the document is built as a tree

Found by the family audit of 2026-11-13 (GCAL-12 and GCAL-02, high; GCAL-11, low).

The stash stored an unmanaged line as `Display` writes it, the wire shape it arrived in, folds included (`raw_line`), joined the lines with a line feed (`chunk_into`) and split on the line feed alone (`stashed`); `to_ical` then joined the pieces with CRLF. A folded line came back as `...\r\r\n ...`, which reads as a value holding a CR, and gained one more CR per fold each time the document went through the projection. `recurrence` was fed by the same writer, so a folded EXDATE was sent to Google as `EXDATE:...090\r\n 000Z,...`.

`to_ical` serialized its tree, then inserted the recurrence lines, the stash and the alarms before the first `END:VEVENT` of the text and the calendar remainder before the first `END:VCALENDAR` (`splice_before`, a substring search); `to_ical_series` inserted the exceptions the same way. An event whose UID is `END:VEVENT` projected to a VEVENT closed before its DTSTART, which `from_ical` refuses; one summarised `END:VEVENT` projected to `SUMMARY:RRULE:...` with its attendees outside the VEVENT, which the strict parser accepts.

What landed, all in src/v3/rest/events/ical.rs:

- `raw_line` builds the logical line from the name, each parameter and the raw value, and drops any CR. `raw_component` walks the component.
- `stashed` drops every CR and re-joins a piece opening on a space or a tab onto the piece before it, so the stashes already stored at Google read whole, the ones a second round trip stored as `\r\r\n ` too. The new shape is the one the former reader took for a line that arrived unfolded, so an older build still reads a stash written by this one.
- `to_ical_series` builds one tree (the VEVENT of the master, the calendar remainder, the VEVENT of each exception) and serializes it once; `to_ical` calls it without exception. `push_lines` tokenises the recurrence lines and the stashed lines into the tree, resets their wire shape so the serializer folds them at 75 octets, leaves out a line that is no content line (traced) and an END closing nothing, and closes a BEGIN left open. The alarms are components. The zones are read from the tree (`sort_zones`, the TZID parameter through its lens rather than a search for `TZID=` in the text) and minted once per document, where each exception used to mint and discard its own. `splice_before`, `define_zones`, `undefined_zones`, `unfolded`, `vevent_lines` and `anchor` are gone, the last folded into its one caller.

Tests, all failing on the former code: `a_folded_line_is_stashed_whole_and_comes_back_without_a_stray_cr` (the stash held `URL:...aaa\r\n aaa...`), `a_folded_recurrence_line_is_sent_unfolded` (`EXDATE:...T090\r\n 000Z,...` in `recurrence`), `a_stash_stored_with_its_folds_reads_back_whole` (`...aaa\r\r\n aaa...` in the document), `any_unmanaged_line_survives_the_round_trip_however_it_is_folded` (500 generated documents: names taken from a list or drawn, up to two parameters, a quoted one outside ASCII among them, values of 0 to 900 octets of one to four octets a character, folds at any octet for half of them and between characters for the other half, with a space or a tab, CRLF or LF, a line nested in a component each time; the former stash of the half folded between characters is rebuilt and must project to the same bytes; case 0 showed `R\r\n ELAT\r\n\tED-T\n O:` where `RELATED-TO:` was expected), `a_text_holding_a_marker_leaves_every_line_in_its_component` (four markers, alone and inside a sentence, in the UID, SUMMARY, DESCRIPTION, LOCATION, the CN of the organizer and of an attendee and the LABEL of a conference, over a series with an exception; `END:VEVENT` as the UID failed with `NoZonedStart`) and `an_unbalanced_stash_never_breaks_the_document` (a stray `END:VEVENT` closed the component early). No existing test changed.

Checked beside the suite, in a scratch crate holding the former file: the fixture, an event with every stash kind, a series and five events shaped as the API returns them project to the same bytes before and after, a stash written by the former code included. Two differences are meant: a line wider than 75 octets coming from `recurrence` or from the stash is now folded, and the VTIMEZONE of a series come in name order (those an exception alone named used to follow the master's).

Left open:

- GCAL-11 is done for the splices and the per-exception zones only. A VTIMEZONE cache across a listing needs a public cache type and a key for the era, which ical-rs does not expose; `Accept-Encoding: gzip` waits on io-http; the `fields` parameter is a feature of its own.
- A logical line over 1024 octets is not stashed, as before, but the width is now that of the logical line everywhere: a property between about 985 and 1024 octets is now kept, and a line of a stashed component over 1024 octets, which used to pass as folded pieces, now stays out, leaving its component without it.
- A CR inside a value is dropped by the stash. No content line may hold one, and the former projection is the known source of those met in the field.
- Not checked live: whether Google returns a CR, or a whitespace opening or ending a chunk, as it was sent. The second matters to the chunking as it always did.
- Who owns the stash, this crate or ical-rs, is undecided; nothing moved.

Capabilities moved: events-ical (added: The stash holds logical lines, A stash stored folded reads back whole, The document is built as a tree, Stored lines stay balanced).
