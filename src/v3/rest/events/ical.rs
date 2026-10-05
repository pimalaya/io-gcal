//! # iCalendar projection
//!
//! Projects a Google Calendar event onto iCalendar and back. The Calendar
//! API exposes no iCalendar form of an event, so a consumer needing one (a
//! calendar client, a sync engine) synthesizes the document of record from
//! the JSON resource ([`GcalEvent::to_ical`], [`GcalEvent::to_ical_series`])
//! and reads it back ([`GcalEvent::from_ical`]).
//!
//! A field is managed only where it has a well-defined iCalendar slot:
//!
//! - provider-only fields (`colorId`, `eventType`, the guest switches, the
//!   birthday, focus-time, out-of-office and working-location blocks) have
//!   none, so [`GcalEvent::merge`] carries them over from the server copy
//!   and an update leaves them untouched;
//! - provider-scoped fields (`htmlLink`, `hangoutLink`) are minted as
//!   read-only `X-GOOGLE-*` properties and consumed on the way back;
//! - the ways of joining a conference (`conferenceData.entryPoints`) are
//!   minted as read-only `CONFERENCE` properties (RFC 7986 5.11), one per
//!   entry point, the server value staying authoritative;
//! - `X-PIMDIR-ONLINE-MEETING:TRUE` (pimdir STORAGE Annex B.1) asks for a
//!   Google Meet: it becomes a conference create request, which Google
//!   honours when the write carries `conferenceDataVersion=1`, and is
//!   never written back;
//! - everything else is stashed verbatim in `extendedProperties.private`
//!   and spliced back on read.
//!
//! `created` and `updated` project onto CREATED and LAST-MODIFIED, but
//! Google stamps them itself, so an incoming CREATED or LAST-MODIFIED is
//! consumed rather than written. Every TZID the document names gets a
//! VTIMEZONE, synthesized from the bundled time zone database.

use core::{fmt, mem::take};

use alloc::{
    borrow::ToOwned,
    collections::{BTreeMap, BTreeSet},
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use ical::{
    component::vevent::VEVENT,
    param::IcalParam,
    prop::{
        IcalProp, IcalPropKind, IcalPropName, action::ACTION, trigger::TRIGGER,
        tzid::TZID as TZID_PROP,
    },
    tree::{
        codec::Codec,
        cst::{IcalCst, IcalItem},
        error::IcalParseError,
        line::IcalLine,
        param::{cn::CN, cutype::CUTYPE, partstat::PARTSTAT, role::ROLE, tzid::TZID, value::VALUE},
    },
    tzdb,
    value::{IcalValue, datetime::IcalDateTime, integer::IcalInteger, text::IcalText},
};
use jiff::{Timestamp, civil::Date, tz::TimeZone};

use crate::v3::rest::{
    calendars::GcalConferenceSolutionType,
    events::{
        GcalConferenceData, GcalConferenceSolutionKey, GcalCreateConferenceRequest, GcalEntryPoint,
        GcalEntryPointType, GcalEvent, GcalEventAttendee, GcalEventAttendeeResponseStatus,
        GcalEventDateTime, GcalEventExtendedProperties, GcalEventPerson, GcalEventReminder,
        GcalEventReminderMethod, GcalEventReminders, GcalEventStatus, GcalEventTransparency,
        GcalEventVisibility,
    },
};

/// Product identifier the synthesized document carries.
const PRODID: &str = "-//Pimalaya//calendula//EN";

/// Widest value Google accepts in an extended property, hence a chunk.
///
/// A single line longer than this stays in the local document, never
/// sent, rather than risking the whole write.
const MAX_STASH_CHUNK: usize = 1024;

/// Key prefix of the chunks stashing the VEVENT remainder.
///
/// The prefixes predate this crate (Calendula wrote them first) and are
/// kept so events stashed then still read back.
const EVENT_STASH_PREFIX: &str = "calendula.ical.";

/// Key prefix of the chunks stashing the VCALENDAR remainder.
///
/// The calendar-level properties and components outside the VEVENT, a
/// VTIMEZONE the event's TZID references most of all.
const CALENDAR_STASH_PREFIX: &str = "calendula.vcal.";

/// Properties [`GcalEvent::to_ical`] mints from the Google-scoped event fields.
///
/// [`GcalEvent::from_ical`] drops them, the server value staying authoritative, so
/// a minted property is neither managed nor part of the remainder.
const MINTED_PROPS: &[&str] = &[
    "X-GOOGLE-HTML-LINK",
    "X-GOOGLE-HANGOUT-LINK",
    "CONFERENCE",
    // NOTE: minted before CONFERENCE was, and still dropped from the
    // documents a store kept since.
    "X-GOOGLE-CONFERENCE",
];

/// The property asking the source for an online meeting of its provider
/// (pimdir STORAGE Annex B.1).
const ONLINE_MEETING_PROP: &str = "X-PIMDIR-ONLINE-MEETING";

/// Calendar-level properties the projection rewrites on every read.
///
/// Never stashed: the rewrite already restores them.
const CALENDAR_OWNED_PROPS: &[&str] = &["VERSION", "PRODID", "CALSCALE"];

/// Google's ceiling on a reminder lead time: four weeks, in minutes.
const MAX_REMINDER_MINUTES: u64 = 40320;

impl GcalEvent {
    /// Projects an io-gcal event onto a fresh VCALENDAR document.
    pub fn to_ical(&self) -> String {
        let event = self;
        let mut vevent = IcalCst::empty("VEVENT");

        let uid = event
            .ical_uid
            .clone()
            .or_else(|| event.id.clone())
            .unwrap_or_default();
        vevent.push(IcalProp::text(IcalPropKind::Uid, vec![], uid));

        // NOTE: DTSTAMP is mandatory (RFC 5545 3.6.1) and Google carries no
        // field of its own for it, so the last modification time stands in.
        let stamp = event
            .updated
            .as_deref()
            .or(event.created.as_deref())
            .and_then(ical_utc);
        if let Some(stamp) = stamp {
            vevent.push(stamp_prop(IcalPropKind::DtStamp, stamp));
        }

        if let Some(start) = &event.start {
            push_boundary(&mut vevent, IcalPropKind::DtStart, start);
        }

        if let Some(end) = &event.end {
            push_boundary(&mut vevent, IcalPropKind::DtEnd, end);
        }

        // NOTE: an exception carries the UID of its series and names the
        // instance it replaces with a RECURRENCE-ID (RFC 5545 3.8.4.4).
        // Without it the two cannot be filed as the one resource RFC 4791
        // 4.1 requires them to share.
        if event.recurring_event_id.is_some()
            && let Some(original) = &event.original_start_time
        {
            push_boundary(&mut vevent, IcalPropKind::RecurrenceId, original);
        }

        for (kind, value) in [
            (IcalPropKind::Summary, &event.summary),
            (IcalPropKind::Description, &event.description),
            (IcalPropKind::Location, &event.location),
        ] {
            if let Some(value) = value {
                vevent.push(IcalProp::text(kind, vec![], value.clone()));
            }
        }

        if let Some(status) = event.status {
            vevent.push(IcalProp::text(
                IcalPropKind::Status,
                vec![],
                status_to_ical(status),
            ));
        }

        if let Some(transparency) = event.transparency {
            vevent.push(IcalProp::text(
                IcalPropKind::Transp,
                vec![],
                transparency_to_ical(transparency),
            ));
        }

        if let Some(class) = event.visibility.and_then(visibility_to_ical) {
            vevent.push(IcalProp::text(IcalPropKind::Class, vec![], class));
        }

        if let Some(sequence) = event.sequence {
            vevent.push(IcalProp {
                name: IcalPropName::Kind(IcalPropKind::Sequence),
                params: Vec::new(),
                value: IcalValue::Integer(IcalInteger(sequence.to_string().into())),
            });
        }

        if let Some(created) = event.created.as_deref().and_then(ical_utc) {
            vevent.push(stamp_prop(IcalPropKind::Created, created));
        }

        if let Some(updated) = event.updated.as_deref().and_then(ical_utc) {
            vevent.push(stamp_prop(IcalPropKind::LastModified, updated));
        }

        if let Some(organizer) = &event.organizer {
            vevent.push(person_prop(organizer));
        }

        for attendee in &event.attendees {
            vevent.push(attendee_prop(attendee));
        }

        for (name, value) in [
            ("X-GOOGLE-HTML-LINK", event.html_link.as_deref()),
            ("X-GOOGLE-HANGOUT-LINK", event.hangout_link.as_deref()),
        ] {
            if let Some(value) = value {
                vevent.push(IcalProp::text(name, vec![], value.to_string()));
            }
        }

        for entry in event
            .conference_data
            .iter()
            .flat_map(|conference| &conference.entry_points)
        {
            if let Some(prop) = conference_prop(entry) {
                vevent.push(prop);
            }
        }

        let mut calendar = IcalCst::v2();
        calendar.push(IcalProp::text(
            IcalPropKind::ProdId,
            vec![],
            PRODID.to_string(),
        ));
        calendar.push_component(vevent);

        // NOTE: the recurrence lines and the stash are already iCalendar
        // syntax, so they are spliced in verbatim; the alarms follow, so the
        // VEVENT keeps its properties before its subcomponents.
        let mut lines = event.recurrence.clone();
        lines.extend(stashed(event, EVENT_STASH_PREFIX));

        let document = splice_before(calendar.to_string(), "END:VEVENT", &lines);
        let document = splice_before(document, "END:VEVENT", &alarms(event));

        let document = splice_before(
            document,
            "END:VCALENDAR",
            &stashed(event, CALENDAR_STASH_PREFIX),
        );

        define_zones(document, anchor(event))
    }

    /// Folds a recurrence set into the one resource RFC 4791 4.1 requires.
    ///
    /// Google is instance-granular where CalDAV is resource-granular, so
    /// without the fold two stores of the same calendar disagree about what
    /// they hold. Each exception keeps the master's UID.
    pub fn to_ical_series(&self, overrides: &[&GcalEvent]) -> String {
        let master = self;
        let document = master.to_ical();

        if overrides.is_empty() {
            return document;
        }

        let uid = master.ical_uid.clone().or_else(|| master.id.clone());

        let mut lines = Vec::new();

        for exception in overrides {
            let mut exception = (*exception).clone();

            // NOTE: the components of one resource share one UID (RFC 4791
            // 4.1), and an exception projected alone would mint its own
            // from its event id when Google returned no iCalUID.
            exception.ical_uid = uid.clone();

            lines.extend(vevent_lines(&exception.to_ical()));
        }

        // NOTE: minted again over the whole document, since an exception
        // moved into another zone names a TZID the master's own definitions
        // never covered.
        define_zones(
            splice_before(document, "END:VCALENDAR", &lines),
            anchor(master),
        )
    }

    /// Projects an iCalendar document back onto an io-gcal event.
    ///
    /// Only the managed fields and the stash are filled: a provider-only
    /// field has no iCalendar source, and [`merge`](Self::merge) carries it over from
    /// the server copy instead.
    pub fn from_ical(contents: &[u8]) -> Result<Self, GcalEventIcalError> {
        let calendar = IcalCst::parse(contents).map_err(GcalEventIcalError::Parse)?;
        let (vevent, calendar) = take_vevent(&calendar)?;

        let mut event = GcalEvent::default();
        let mut reminders = Vec::new();
        let mut stash = Vec::new();
        let mut online_meeting = false;
        let mut stamp = String::new();

        for item in &vevent.items {
            match item {
                IcalItem::Prop(line) => {
                    let name = line.name.get();

                    if name.eq_ignore_ascii_case(ONLINE_MEETING_PROP) {
                        online_meeting = line.raw_value_str().trim().eq_ignore_ascii_case("TRUE");
                        continue;
                    }

                    if name.eq_ignore_ascii_case("DTSTAMP") {
                        stamp = line.raw_value_str().trim().to_string();
                    }

                    if !consume_prop(&mut event, line) {
                        stash.push(raw_line(line));
                    }
                }
                IcalItem::Component(child) if is_named(child, "VALARM") => match reminder(child) {
                    Some(reminder) => reminders.push(reminder),
                    None => stash.extend(raw_component(child)),
                },
                IcalItem::Component(child) => stash.extend(raw_component(child)),
                IcalItem::Opaque(bytes) => stash.push(String::from_utf8_lossy(bytes).into_owned()),
            }
        }

        if event.start.is_none() {
            return Err(GcalEventIcalError::NoZonedStart);
        }

        if event.end.is_none() {
            return Err(GcalEventIcalError::NoEnd);
        }

        // NOTE: an empty override list with the defaults turned off means
        // "no reminder at all", so a document carrying no VALARM inherits
        // the calendar's defaults rather than silencing the event.
        event.reminders = Some(GcalEventReminders {
            use_default: Some(reminders.is_empty()),
            overrides: reminders,
        });

        // NOTE: Google expands a recurrence in the time zone of the start,
        // and a UTC offset does not name one, so a recurring event whose
        // boundaries are UTC has to say so explicitly.
        if !event.recurrence.is_empty() {
            for boundary in [event.start.as_mut(), event.end.as_mut()]
                .into_iter()
                .flatten()
            {
                if boundary.is_timed_without_time_zone() {
                    boundary.time_zone = Some(String::from("UTC"));
                }
            }
        }

        if online_meeting {
            event.conference_data = Some(meet_request(
                event.ical_uid.as_deref().unwrap_or_default(),
                &stamp,
            ));
        }

        let calendar_stash = calendar.map(calendar_remainder).unwrap_or_default();

        let mut private = BTreeMap::new();
        chunk_into(&mut private, EVENT_STASH_PREFIX, &stash);
        chunk_into(&mut private, CALENDAR_STASH_PREFIX, &calendar_stash);

        if !private.is_empty() {
            event.extended_properties = Some(GcalEventExtendedProperties {
                private,
                shared: BTreeMap::new(),
            });
        }

        Ok(event)
    }

    /// Merges a projected event onto the one the server currently holds.
    ///
    /// A full replacement write must keep what the projection does not
    /// model: provider-only fields come from `current`, managed ones from
    /// `projected` and are authoritative, so a dropped property clears its
    /// field. The stash rewrites only calendula's own key prefixes.
    pub fn merge(self, current: &Self) -> Self {
        let mut projected = self;
        let mut private: BTreeMap<String, String> = current
            .extended_properties
            .iter()
            .flat_map(|properties| properties.private.iter())
            .filter(|(key, _)| !is_stash_key(key))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();

        private.extend(
            projected
                .extended_properties
                .take()
                .into_iter()
                .flat_map(|properties| properties.private),
        );

        let shared = current
            .extended_properties
            .as_ref()
            .map(|properties| properties.shared.clone())
            .unwrap_or_default();

        if !private.is_empty() || !shared.is_empty() {
            projected.extended_properties = Some(GcalEventExtendedProperties { private, shared });
        }

        // NOTE: a projection never carries the conference itself, only a
        // create request when the document asks for a meeting. The one the
        // server holds stays, as does a request still pending: a write with
        // `conferenceDataVersion=1` would drop either if the payload left
        // it out.
        projected.conference_data = match current.conference_data.clone() {
            Some(conference)
                if !conference.entry_points.is_empty() || projected.conference_data.is_none() =>
            {
                Some(conference)
            }
            _ => projected.conference_data.take(),
        };

        carry_display_zone(&mut projected.start, current.start.as_ref());
        carry_display_zone(&mut projected.end, current.end.as_ref());

        GcalEvent {
            // NOTE: Google stamps these itself and ignores an attempt to set
            // them; they ride along so the payload stays recognizable.
            id: current.id.clone(),
            created: current.created.clone(),
            updated: current.updated.clone(),

            // NOTE: the provider-only fields have no iCalendar slot, so
            // they come from the server copy and the write leaves them
            // standing.
            color_id: current.color_id.clone(),
            event_label_id: current.event_label_id.clone(),
            event_type: current.event_type,
            anyone_can_add_self: current.anyone_can_add_self,
            guests_can_invite_others: current.guests_can_invite_others,
            guests_can_modify: current.guests_can_modify,
            guests_can_see_other_guests: current.guests_can_see_other_guests,
            birthday_properties: current.birthday_properties.clone(),
            focus_time_properties: current.focus_time_properties.clone(),
            out_of_office_properties: current.out_of_office_properties.clone(),
            working_location_properties: current.working_location_properties.clone(),
            gadget: current.gadget.clone(),
            source: current.source.clone(),
            attachments: current.attachments.clone(),

            ..projected
        }
    }
}

/// An iCalendar document that cannot become a Google event.
#[derive(Debug)]
pub enum GcalEventIcalError {
    /// The document does not parse.
    Parse(IcalParseError),
    /// The VEVENT starts neither in UTC nor in a named zone.
    NoZonedStart,
    /// The VEVENT has no DTEND, which Google requires.
    NoEnd,
    /// The document holds a component Google does not model.
    NotAnEvent(String),
    /// The document holds no VEVENT.
    NoEvent,
}

impl fmt::Display for GcalEventIcalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(err) => write!(f, "Parse iCalendar: {err}"),
            Self::NoZonedStart => {
                write!(
                    f,
                    "Google needs a DTSTART carrying either a UTC `Z` suffix or a TZID"
                )
            }
            Self::NoEnd => {
                write!(
                    f,
                    "Google needs a DTEND on every event; a DURATION alone is not enough"
                )
            }
            Self::NotAnEvent(name) => {
                write!(f, "Google models no {name}: its calendars hold events only")
            }
            Self::NoEvent => write!(f, "The iCalendar contents carry no VEVENT"),
        }
    }
}

impl core::error::Error for GcalEventIcalError {}

/// Mints a VTIMEZONE for every zone the document names without defining.
///
/// RFC 5545 3.2.19 makes every TZID owe one, and Google's resource holds
/// the zone name alone. Minted last, since a TZID also reaches the
/// document through the recurrence lines, the stash and a folded
/// exception.
fn define_zones(document: String, anchor: i64) -> String {
    let definitions: Vec<String> = undefined_zones(&document)
        .iter()
        .filter_map(|zone| tzdb::vtimezone(zone, anchor))
        .flat_map(|definition| raw_component(&definition))
        .collect();

    // NOTE: ahead of the VEVENT rather than at the end of the envelope,
    // where Google's own CalDAV frontend puts it: a reader taking the
    // document a line at a time then meets a definition before the
    // property leaning on it.
    splice_before(document, "BEGIN:VEVENT", &definitions)
}

/// The VEVENT block of a projected document, BEGIN and END included.
///
/// Verbatim, so a folded exception keeps the wire form just written.
fn vevent_lines(document: &str) -> Vec<String> {
    let mut lines: Vec<String> = document
        .lines()
        .map(|line| line.trim_end_matches('\r'))
        .skip_while(|line| !line.starts_with("BEGIN:VEVENT"))
        .take_while(|line| !line.starts_with("END:VEVENT"))
        .map(str::to_string)
        .collect();

    // NOTE: closes the block take_while stopped at, but a document
    // holding no VEVENT must contribute nothing rather than a stray END.
    if !lines.is_empty() {
        lines.push(String::from("END:VEVENT"));
    }

    lines
}

/// Every zone the document names in a TZID parameter without defining
/// it in a VTIMEZONE of its own.
fn undefined_zones(document: &str) -> BTreeSet<String> {
    let mut referenced = BTreeSet::new();
    let mut defined = BTreeSet::new();

    for line in unfolded(document) {
        if let Some(zone) = line.strip_prefix("TZID:") {
            defined.insert(zone.trim().to_string());
            continue;
        }

        // NOTE: only the parameter section names a zone. Searching the
        // whole line would let a SUMMARY reading `TZID=` conjure a
        // component out of free text.
        let params = line.split(':').next().unwrap_or(&line);

        let Some(start) = params.find("TZID=") else {
            continue;
        };

        let zone = &params[start + "TZID=".len()..];
        let end = zone.find(';').unwrap_or(zone.len());
        referenced.insert(zone[..end].trim_matches('"').to_string());
    }

    referenced.difference(&defined).cloned().collect()
}

/// The document's logical lines, RFC 5545 3.1 folds resolved.
///
/// A folded line is one line split across several physical ones, so
/// reading the physical ones would see a zone name cut in half.
fn unfolded(document: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();

    for line in document.lines() {
        let line = line.trim_end_matches('\r');
        let continuation = line.strip_prefix([' ', '\t']);

        match (continuation, lines.last_mut()) {
            (Some(rest), Some(last)) => last.push_str(rest),
            _ => lines.push(line.to_string()),
        }
    }

    lines
}

/// The instant an event's zones are described around, in Unix seconds.
///
/// Only the era matters, observances changing on the scale of years, so
/// the start date alone is read, as UTC, off by a day at worst. An
/// event with no start is refused on write, so the epoch never ships.
fn anchor(event: &GcalEvent) -> i64 {
    let boundary = event
        .start
        .as_ref()
        .and_then(|boundary| boundary.date_time.as_deref().or(boundary.date.as_deref()));

    boundary
        .and_then(|stamp| stamp.get(..10))
        .and_then(|date| date.parse::<Date>().ok())
        .and_then(|date| date.to_zoned(TimeZone::UTC).ok())
        .map_or(0, |zoned| zoned.timestamp().as_second())
}

/// Carries a boundary's display zone over from the server copy.
///
/// Google returns an absolute instant plus the calendar's display zone,
/// which no iCalendar boundary expresses, so the projection emits the
/// stamp alone and loses the zone. A series expands in the zone of its
/// start, and would drift an hour after a daylight-saving change.
///
/// Only a UTC-stamped instant may take the zone: the stamp already
/// fixes the instant, so the label moves nothing, while relabelling an
/// offset-less wall time would shift the event.
fn carry_display_zone(
    projected: &mut Option<GcalEventDateTime>,
    current: Option<&GcalEventDateTime>,
) {
    let Some(boundary) = projected else { return };
    let Some(zone) = current.and_then(|boundary| boundary.time_zone.as_deref()) else {
        return;
    };
    let Some(date_time) = boundary.date_time.as_deref() else {
        return;
    };

    if is_utc_stamped(date_time) {
        boundary.time_zone = Some(zone.to_owned());
    }
}

/// Whether an extended property key belongs to a calendula stash chunk.
fn is_stash_key(key: &str) -> bool {
    key.starts_with(EVENT_STASH_PREFIX) || key.starts_with(CALENDAR_STASH_PREFIX)
}

/// The VEVENT of a parsed document, and the calendar wrapping it if any.
///
/// A document carrying none is refused by the component name it does
/// carry: Google models no VTODO or VJOURNAL, and emulating one would
/// store what no other client could read back.
fn take_vevent<'a>(
    calendar: &'a IcalCst<'a>,
) -> Result<(&'a IcalCst<'a>, Option<&'a IcalCst<'a>>), GcalEventIcalError> {
    if is_named(calendar, "VEVENT") {
        return Ok((calendar, None));
    }

    if let Some(vevent) = calendar.component::<VEVENT>() {
        return Ok((vevent, Some(calendar)));
    }

    let found = calendar.items.iter().find_map(|item| match item {
        IcalItem::Component(child) => Some(component_name(child).to_uppercase()),
        _ => None,
    });

    match found {
        Some(name) => Err(GcalEventIcalError::NotAnEvent(name)),
        None => Err(GcalEventIcalError::NoEvent),
    }
}

/// The VCALENDAR-level remainder the projection does not rewrite.
///
/// A VTIMEZONE the database knows is left out: stashing a dozen lines
/// would spend the extended-property budget on bytes the projection
/// mints for free. One it does not know is kept verbatim, since nothing
/// else could rebuild it.
fn calendar_remainder(calendar: &IcalCst<'_>) -> Vec<String> {
    let mut remainder = Vec::new();

    for item in &calendar.items {
        match item {
            IcalItem::Prop(line) if !is_calendar_owned(line.name.get()) => {
                remainder.push(raw_line(line))
            }
            IcalItem::Prop(_) => {}
            IcalItem::Component(child) if is_named(child, "VEVENT") => {}
            IcalItem::Component(child) if is_known_zone(child) => {}
            IcalItem::Component(child) => remainder.extend(raw_component(child)),
            IcalItem::Opaque(bytes) => remainder.push(String::from_utf8_lossy(bytes).into_owned()),
        }
    }

    remainder
}

/// Whether a component is a VTIMEZONE mintable from its TZID alone.
fn is_known_zone(component: &IcalCst<'_>) -> bool {
    if !is_named(component, "VTIMEZONE") {
        return false;
    }

    component
        .prop::<TZID_PROP>()
        .is_some_and(|tzid| tzdb::is_known(&tzid.0))
}

/// Whether a calendar-level property is one the projection rewrites.
fn is_calendar_owned(name: &str) -> bool {
    CALENDAR_OWNED_PROPS
        .iter()
        .any(|owned| name.eq_ignore_ascii_case(owned))
}

/// Reads one VEVENT property into the event, reporting whether it was
/// consumed.
///
/// An unconsumed property lands in the stash.
fn consume_prop(event: &mut GcalEvent, line: &IcalLine<'_>) -> bool {
    let name = line.name.get();

    if MINTED_PROPS
        .iter()
        .any(|minted| name.eq_ignore_ascii_case(minted))
    {
        return true;
    }

    let Ok(kind) = name.parse::<IcalPropKind>() else {
        return false;
    };

    match kind {
        IcalPropKind::Uid => {
            event.ical_uid = Some(text(line));
            true
        }
        IcalPropKind::Summary => {
            event.summary = Some(text(line));
            true
        }
        IcalPropKind::Description => {
            event.description = Some(text(line));
            true
        }
        IcalPropKind::Location => {
            event.location = Some(text(line));
            true
        }
        IcalPropKind::DtStart => {
            event.start = boundary(line);
            event.start.is_some()
        }
        IcalPropKind::DtEnd => {
            event.end = boundary(line);
            event.end.is_some()
        }
        IcalPropKind::Status => {
            event.status = status_from_ical(&line.raw_value_str());
            event.status.is_some()
        }
        IcalPropKind::Transp => {
            event.transparency = transparency_from_ical(&line.raw_value_str());
            event.transparency.is_some()
        }
        IcalPropKind::Class => {
            event.visibility = visibility_from_ical(&line.raw_value_str());
            event.visibility.is_some()
        }
        IcalPropKind::Sequence => {
            event.sequence = line.raw_value_str().trim().parse().ok();
            event.sequence.is_some()
        }
        IcalPropKind::Organizer => {
            event.organizer = Some(person(line));
            true
        }
        IcalPropKind::Attendee => {
            event.attendees.push(attendee(line));
            true
        }
        IcalPropKind::RRule | IcalPropKind::ExRule | IcalPropKind::RDate | IcalPropKind::ExDate => {
            event.recurrence.push(raw_line(line));
            true
        }
        // NOTE: Google stamps these three itself, so an incoming value
        // is consumed rather than stashed and never written back.
        IcalPropKind::DtStamp | IcalPropKind::Created | IcalPropKind::LastModified => true,
        _ => false,
    }
}

/// The CONFERENCE property of one way of joining a conference.
///
/// RFC 7986 5.11 requires the URI value type and allows one property per
/// entry point, each saying what it offers in FEATURE. An entry point
/// without a URI projects to nothing.
fn conference_prop(entry: &GcalEntryPoint) -> Option<IcalProp<'static>> {
    let uri = entry.uri.as_deref().filter(|uri| !uri.is_empty())?;
    let mut params = vec![IcalParam::Value("URI".into())];

    let features: &[&str] = match entry.entry_point_type {
        Some(GcalEntryPointType::Video) => &["AUDIO", "VIDEO"],
        Some(GcalEntryPointType::Phone) => &["PHONE"],
        Some(GcalEntryPointType::Sip) => &["AUDIO"],
        Some(GcalEntryPointType::More) | None => &[],
    };

    if !features.is_empty() {
        params.push(IcalParam::Feature(
            features.iter().map(|feature| (*feature).into()).collect(),
        ));
    }

    if let Some(label) = entry.label.as_deref().filter(|label| !label.is_empty()) {
        params.push(IcalParam::Label(label.to_string().into()));
    }

    Some(IcalProp {
        name: IcalPropName::Kind(IcalPropKind::Conference),
        params,
        value: IcalValue::Uri(uri.to_string().into()),
    })
}

/// A request for a new Google Meet attached to the event.
///
/// Google replays the outcome of a request id it has already seen, so the
/// id is drawn from the document (UID and DTSTAMP): pushing the same
/// document again never creates a second meeting.
fn meet_request(uid: &str, stamp: &str) -> GcalConferenceData {
    // NOTE: FNV-1a, enough to keep the id short and stable; it guards
    // against nothing but a replay.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;

    for byte in uid.bytes().chain([0]).chain(stamp.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }

    GcalConferenceData {
        create_request: Some(GcalCreateConferenceRequest {
            request_id: Some(format!("pimdir-{hash:016x}")),
            conference_solution_key: Some(GcalConferenceSolutionKey {
                solution_type: Some(GcalConferenceSolutionType::HangoutsMeet),
            }),
            status: None,
        }),
        ..GcalConferenceData::default()
    }
}

/// The VALARM blocks projected from the event's reminder overrides.
///
/// A reminder missing its method or its lead time projects to nothing,
/// and a set is projected only when it overrides the calendar's
/// defaults, which belong to the calendar and not to this event.
fn alarms(event: &GcalEvent) -> Vec<String> {
    event
        .reminders
        .iter()
        .filter(|reminders| reminders.use_default != Some(true))
        .flat_map(|reminders| &reminders.overrides)
        .filter_map(|reminder| {
            let action = match reminder.method? {
                GcalEventReminderMethod::Email => "EMAIL",
                GcalEventReminderMethod::Popup => "DISPLAY",
            };
            let minutes = reminder.minutes?;

            Some(vec![
                String::from("BEGIN:VALARM"),
                format!("ACTION:{action}"),
                format!("TRIGGER:-PT{minutes}M"),
                String::from("DESCRIPTION:Reminder"),
                String::from("END:VALARM"),
            ])
        })
        .flatten()
        .collect()
}

/// Projects a VALARM back onto a Google reminder override.
///
/// `None` when Google cannot model it: an action that is neither
/// display nor email, or a trigger that is not a lead time in whole
/// minutes, stays in the stash rather than being flattened.
fn reminder(alarm: &IcalCst<'_>) -> Option<GcalEventReminder> {
    let action = alarm.prop::<ACTION>()?;
    let method = match action.0.trim().to_uppercase().as_str() {
        "DISPLAY" => GcalEventReminderMethod::Popup,
        "EMAIL" => GcalEventReminderMethod::Email,
        _ => return None,
    };

    let trigger = alarm.prop::<TRIGGER>()?;
    let minutes = lead_minutes(trigger.0.trim())?;

    Some(GcalEventReminder {
        method: Some(method),
        minutes: Some(minutes),
    })
}

/// The whole minutes of lead time a negative RFC 5545 duration names.
///
/// Anything else (a positive trigger, a sub-minute remainder, one past
/// Google's four-week ceiling) returns `None`, and the alarm stays in
/// the stash.
fn lead_minutes(duration: &str) -> Option<u32> {
    let rest = duration.strip_prefix('-')?.strip_prefix('P')?;
    let (date, time) = match rest.split_once('T') {
        Some((date, time)) => (date, time),
        None => (rest, ""),
    };

    let mut seconds: u64 = 0;
    let mut digits = String::new();

    for (part, units) in [(date, "WD"), (time, "HMS")] {
        for character in part.chars() {
            if character.is_ascii_digit() {
                digits.push(character);
                continue;
            }

            if !units.contains(character) || digits.is_empty() {
                return None;
            }

            let value: u64 = digits.parse().ok()?;
            digits.clear();

            seconds += value
                * match character {
                    'W' => 7 * 24 * 3600,
                    'D' => 24 * 3600,
                    'H' => 3600,
                    'M' => 60,
                    _ => 1,
                };
        }
    }

    if !digits.is_empty() || !seconds.is_multiple_of(60) {
        return None;
    }

    let minutes = seconds / 60;
    (minutes <= MAX_REMINDER_MINUTES).then_some(minutes as u32)
}

/// Reads a DTSTART or DTEND line into a Google boundary.
///
/// A floating stamp has no Google form, the API needing an offset or a
/// named zone, so it is left unconsumed rather than guessing one, and
/// the write then fails by name on the missing boundary.
fn boundary(line: &IcalLine<'_>) -> Option<GcalEventDateTime> {
    let value = line.raw_value_str();
    let value = value.trim();

    let is_date = line
        .param::<VALUE>()
        .map(|kind| kind.eq_ignore_ascii_case("DATE"))
        .unwrap_or(false)
        || (value.len() == 8 && value.bytes().all(|byte| byte.is_ascii_digit()));

    if is_date {
        return Some(GcalEventDateTime {
            date: Some(rfc3339_date(value)?),
            ..Default::default()
        });
    }

    let zone = line.param::<TZID>().map(|zone| zone.into_owned());

    match (value.strip_suffix('Z'), zone) {
        (Some(local), _) => Some(GcalEventDateTime {
            date_time: Some(format!("{}Z", rfc3339_local(local)?)),
            ..Default::default()
        }),
        (None, Some(zone)) => Some(GcalEventDateTime {
            date_time: Some(rfc3339_local(value)?),
            time_zone: Some(zone),
            ..Default::default()
        }),
        (None, None) => None,
    }
}

/// Pushes a DTSTART or DTEND line for a Google boundary.
///
/// A `VALUE=DATE` property for an all-day one, a UTC stamp for a timed
/// one carrying an offset, a `TZID` stamp for a named zone.
fn push_boundary(vevent: &mut IcalCst<'static>, kind: IcalPropKind, boundary: &GcalEventDateTime) {
    if let Some(date) = &boundary.date {
        let Some(stamp) = ical_date(date) else {
            return;
        };

        vevent.push(IcalProp {
            name: IcalPropName::Kind(kind),
            params: vec![IcalParam::Value("DATE".into())],
            value: IcalValue::Date(stamp.into()),
        });
        return;
    }

    let Some(date_time) = &boundary.date_time else {
        return;
    };

    let zone = boundary.time_zone.as_deref();
    let named = zone.filter(|zone| !zone.eq_ignore_ascii_case("UTC"));

    // NOTE: Google renders a zoned boundary in that zone's own offset,
    // so the literal time is its wall time and keeps its TZID: a series
    // expands in the zone of its start, and dropping the name would
    // expand it in UTC and drift an hour across a daylight-saving change.
    if let Some(zone) = named
        && !is_utc_stamped(date_time)
        && let Some(stamp) = ical_local(date_time)
    {
        vevent.push(IcalProp {
            name: IcalPropName::Kind(kind),
            params: vec![IcalParam::TzId(zone.to_owned().into())],
            value: IcalValue::DateTime(stamp.into()),
        });
        return;
    }

    // NOTE: Google returns a `Z`-stamped instant alongside a named
    // `timeZone` whenever the event was written in UTC. That name is the
    // display zone, not the stamp's wall time, so the instant wins:
    // relabelling would shift it, and deriving wall time needs a database.
    if let Some(stamp) = ical_utc(date_time) {
        vevent.push(stamp_prop(kind, stamp));
        return;
    }

    // NOTE: no offset to resolve the instant with. A named zone was
    // handled above, so this is UTC when the boundary says so, and
    // floating otherwise, which a write then refuses by name.
    if let Some(stamp) = ical_local(date_time) {
        let stamp = match zone {
            Some(_) => format!("{stamp}Z"),
            None => stamp,
        };
        vevent.push(stamp_prop(kind, stamp));
    }
}

/// Whether an RFC 3339 timestamp is stamped in UTC.
///
/// Only the offset is consulted, never an accompanying zone name: the
/// two answer different questions, and this one is about the instant.
fn is_utc_stamped(date_time: &str) -> bool {
    date_time
        .rsplit_once('T')
        .is_some_and(|(_, time)| time.ends_with(['Z', 'z']))
}

/// `YYYYMMDD` to the RFC 3339 `yyyy-mm-dd` Google reads.
fn rfc3339_date(value: &str) -> Option<String> {
    if value.len() != 8 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    Some(format!("{}-{}-{}", &value[..4], &value[4..6], &value[6..]))
}

/// `YYYYMMDDTHHMMSS` to `yyyy-mm-ddThh:mm:ss`, the offset-less RFC 3339
/// local form Google reads alongside a named time zone.
fn rfc3339_local(value: &str) -> Option<String> {
    let (date, time) = value.split_once('T')?;

    if time.len() != 6 || !time.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    Some(format!(
        "{}T{}:{}:{}",
        rfc3339_date(date)?,
        &time[..2],
        &time[2..4],
        &time[4..]
    ))
}

/// `yyyy-mm-dd` to the iCalendar `YYYYMMDD`.
fn ical_date(date: &str) -> Option<String> {
    let stamp: String = date.chars().filter(char::is_ascii_digit).collect();
    (stamp.len() == 8).then_some(stamp)
}

/// An RFC 3339 timestamp to the iCalendar local `YYYYMMDDTHHMMSS`.
///
/// Whatever offset it carries is dropped: the TZID parameter names the
/// zone instead.
fn ical_local(date_time: &str) -> Option<String> {
    let (date, time) = date_time.split_once('T')?;
    let date = ical_date(date)?;
    let time: String = time
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == ':')
        .filter(char::is_ascii_digit)
        .collect();

    (time.len() >= 6).then(|| format!("{date}T{}", &time[..6]))
}

/// An RFC 3339 timestamp to the iCalendar UTC form `YYYYMMDDTHHMMSSZ`.
fn ical_utc(date_time: &str) -> Option<String> {
    let stamp = date_time.parse::<Timestamp>().ok()?;
    Some(stamp.strftime("%Y%m%dT%H%M%SZ").to_string())
}

/// Projects a Google event status onto its STATUS value.
fn status_to_ical(status: GcalEventStatus) -> String {
    match status {
        GcalEventStatus::Confirmed => "CONFIRMED",
        GcalEventStatus::Tentative => "TENTATIVE",
        GcalEventStatus::Cancelled => "CANCELLED",
    }
    .to_string()
}

/// Reads a STATUS value back onto a Google event status.
fn status_from_ical(value: &str) -> Option<GcalEventStatus> {
    match value.trim().to_uppercase().as_str() {
        "CONFIRMED" => Some(GcalEventStatus::Confirmed),
        "TENTATIVE" => Some(GcalEventStatus::Tentative),
        "CANCELLED" => Some(GcalEventStatus::Cancelled),
        _ => None,
    }
}

/// Projects a Google transparency onto its TRANSP value.
fn transparency_to_ical(transparency: GcalEventTransparency) -> String {
    match transparency {
        GcalEventTransparency::Opaque => "OPAQUE",
        GcalEventTransparency::Transparent => "TRANSPARENT",
    }
    .to_string()
}

/// Reads a TRANSP value back onto a Google transparency.
fn transparency_from_ical(value: &str) -> Option<GcalEventTransparency> {
    match value.trim().to_uppercase().as_str() {
        "OPAQUE" => Some(GcalEventTransparency::Opaque),
        "TRANSPARENT" => Some(GcalEventTransparency::Transparent),
        _ => None,
    }
}

/// Projects a Google visibility onto its CLASS value.
///
/// The default visibility is the absence of a CLASS, not a value.
fn visibility_to_ical(visibility: GcalEventVisibility) -> Option<String> {
    match visibility {
        GcalEventVisibility::Default => None,
        GcalEventVisibility::Public => Some(String::from("PUBLIC")),
        GcalEventVisibility::Private => Some(String::from("PRIVATE")),
        GcalEventVisibility::Confidential => Some(String::from("CONFIDENTIAL")),
    }
}

/// Reads a CLASS value back onto a Google visibility.
fn visibility_from_ical(value: &str) -> Option<GcalEventVisibility> {
    match value.trim().to_uppercase().as_str() {
        "PUBLIC" => Some(GcalEventVisibility::Public),
        "PRIVATE" => Some(GcalEventVisibility::Private),
        "CONFIDENTIAL" => Some(GcalEventVisibility::Confidential),
        _ => None,
    }
}

/// An ORGANIZER property from a Google person.
fn person_prop(person: &GcalEventPerson) -> IcalProp<'static> {
    IcalProp {
        name: IcalPropName::Kind(IcalPropKind::Organizer),
        params: person
            .display_name
            .clone()
            .map(|name| IcalParam::Cn(name.into()))
            .into_iter()
            .collect(),
        value: IcalValue::CalAddress(mailto(person.email.as_deref()).into()),
    }
}

/// An ATTENDEE property from a Google attendee.
fn attendee_prop(attendee: &GcalEventAttendee) -> IcalProp<'static> {
    let mut params = Vec::new();

    if let Some(name) = &attendee.display_name {
        params.push(IcalParam::Cn(name.clone().into()));
    }

    if let Some(status) = attendee.response_status {
        params.push(IcalParam::PartStat(partstat_to_ical(status).into()));
    }

    params.push(IcalParam::Role(
        if attendee.optional == Some(true) {
            "OPT-PARTICIPANT"
        } else {
            "REQ-PARTICIPANT"
        }
        .into(),
    ));

    if attendee.resource == Some(true) {
        params.push(IcalParam::CuType("RESOURCE".into()));
    }

    IcalProp {
        name: IcalPropName::Kind(IcalPropKind::Attendee),
        params,
        value: IcalValue::CalAddress(mailto(attendee.email.as_deref()).into()),
    }
}

/// Reads an ORGANIZER line back onto a Google person.
fn person(line: &IcalLine<'_>) -> GcalEventPerson {
    GcalEventPerson {
        email: email(line),
        display_name: line.param::<CN>().map(|name| name.into_owned()),
        ..Default::default()
    }
}

/// Reads an ATTENDEE line back onto a Google attendee.
fn attendee(line: &IcalLine<'_>) -> GcalEventAttendee {
    let role = line.param::<ROLE>().unwrap_or_default();
    let user_type = line.param::<CUTYPE>().unwrap_or_default();

    GcalEventAttendee {
        email: email(line),
        display_name: line.param::<CN>().map(|name| name.into_owned()),
        optional: role.eq_ignore_ascii_case("OPT-PARTICIPANT").then_some(true),
        resource: user_type.eq_ignore_ascii_case("RESOURCE").then_some(true),
        response_status: line
            .param::<PARTSTAT>()
            .and_then(|status| partstat_from_ical(&status)),
        ..Default::default()
    }
}

/// The address a calendar-user-address line carries, `mailto:` stripped.
fn email(line: &IcalLine<'_>) -> Option<String> {
    let value = line.raw_value_str();
    let value = value.trim();
    let address = value.strip_prefix("mailto:").unwrap_or(value);

    (!address.is_empty()).then(|| address.to_string())
}

/// A `mailto:` calendar user address, empty when there is none.
fn mailto(address: Option<&str>) -> String {
    match address {
        Some(address) => format!("mailto:{address}"),
        None => String::new(),
    }
}

/// Projects a Google attendee response onto its PARTSTAT value.
fn partstat_to_ical(status: GcalEventAttendeeResponseStatus) -> String {
    match status {
        GcalEventAttendeeResponseStatus::NeedsAction => "NEEDS-ACTION",
        GcalEventAttendeeResponseStatus::Declined => "DECLINED",
        GcalEventAttendeeResponseStatus::Tentative => "TENTATIVE",
        GcalEventAttendeeResponseStatus::Accepted => "ACCEPTED",
    }
    .to_string()
}

/// Reads a PARTSTAT value back onto a Google attendee response.
fn partstat_from_ical(value: &str) -> Option<GcalEventAttendeeResponseStatus> {
    match value.trim().to_uppercase().as_str() {
        "NEEDS-ACTION" => Some(GcalEventAttendeeResponseStatus::NeedsAction),
        "DECLINED" => Some(GcalEventAttendeeResponseStatus::Declined),
        "TENTATIVE" => Some(GcalEventAttendeeResponseStatus::Tentative),
        "ACCEPTED" => Some(GcalEventAttendeeResponseStatus::Accepted),
        _ => None,
    }
}

/// The stashed lines under `prefix`, chunks reassembled in key order.
fn stashed(event: &GcalEvent, prefix: &str) -> Vec<String> {
    let Some(properties) = &event.extended_properties else {
        return Vec::new();
    };

    let mut chunks: Vec<(u32, &str)> = properties
        .private
        .iter()
        .filter_map(|(key, value)| {
            let index = key.strip_prefix(prefix)?.parse().ok()?;
            Some((index, value.as_str()))
        })
        .collect();
    chunks.sort_by_key(|(index, _)| *index);

    let joined: String = chunks.into_iter().map(|(_, value)| value).collect();

    if joined.is_empty() {
        return Vec::new();
    }

    joined.split('\n').map(str::to_string).collect()
}

/// Chunks the remainder into numbered extended properties under `prefix`.
///
/// A line wider than a whole chunk would risk the write on its own, so
/// it stays in the local document and is never sent.
fn chunk_into(private: &mut BTreeMap<String, String>, prefix: &str, lines: &[String]) {
    let kept: Vec<&str> = lines
        .iter()
        .map(String::as_str)
        .filter(|line| line.len() <= MAX_STASH_CHUNK)
        .collect();

    if kept.is_empty() {
        return;
    }

    let mut chunk = String::new();
    let mut index = 0;

    for character in kept.join("\n").chars() {
        if chunk.len() + character.len_utf8() > MAX_STASH_CHUNK {
            private.insert(format!("{prefix}{index}"), take(&mut chunk));
            index += 1;
        }
        chunk.push(character);
    }

    if !chunk.is_empty() {
        private.insert(format!("{prefix}{index}"), chunk);
    }
}

/// Splices raw lines into a serialized document, right before `marker`.
fn splice_before(document: String, marker: &str, lines: &[String]) -> String {
    if lines.is_empty() {
        return document;
    }

    let mut extra = lines.join("\r\n");
    extra.push_str("\r\n");

    match document.find(marker) {
        Some(position) => {
            let mut out = document;
            out.insert_str(position, &extra);
            out
        }
        None => document + &extra,
    }
}

/// The wire name of a component: the value of its BEGIN line.
fn component_name(component: &IcalCst<'_>) -> String {
    component
        .begin
        .as_ref()
        .map(|begin| begin.raw_value_str().trim().to_string())
        .unwrap_or_default()
}

/// Whether a component's BEGIN line names it `name`.
fn is_named(component: &IcalCst<'_>, name: &str) -> bool {
    component_name(component).eq_ignore_ascii_case(name)
}

/// A UTC date-time property.
fn stamp_prop(kind: IcalPropKind, stamp: String) -> IcalProp<'static> {
    IcalProp {
        name: kind.into(),
        params: Vec::new(),
        value: IcalValue::DateTime(IcalDateTime(stamp.into())),
    }
}

/// The decoded text of a property line, escapes resolved.
fn text(line: &IcalLine<'_>) -> String {
    IcalText::decode(&line.value).0.into_owned()
}

/// One logical line, its ending stripped, ready for the stash.
fn raw_line(line: &IcalLine<'_>) -> String {
    line.to_string().trim_end_matches(['\r', '\n']).to_string()
}

/// A whole component as its raw lines, endings stripped.
fn raw_component(component: &IcalCst<'_>) -> Vec<String> {
    component
        .to_string()
        .lines()
        .map(|line| line.trim_end_matches('\r').to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One of every managed shape, plus a property (CATEGORIES) and a
    /// component (VLOCATION) no Google field models.
    const CALENDAR: &str = concat!(
        "BEGIN:VCALENDAR\r\n",
        "VERSION:2.0\r\n",
        "PRODID:-//Pimalaya//calendula//EN\r\n",
        "X-WR-CALNAME:Work\r\n",
        "BEGIN:VEVENT\r\n",
        "UID:event-1@example.org\r\n",
        "DTSTAMP:20260101T000000Z\r\n",
        "DTSTART:20260814T090000Z\r\n",
        "DTEND:20260814T100000Z\r\n",
        "SUMMARY:Stand-up\r\n",
        "DESCRIPTION:Daily\r\n",
        "LOCATION:Room 2\r\n",
        "STATUS:CONFIRMED\r\n",
        "TRANSP:TRANSPARENT\r\n",
        "CLASS:PRIVATE\r\n",
        "SEQUENCE:3\r\n",
        "ORGANIZER;CN=Alice:mailto:alice@example.org\r\n",
        "ATTENDEE;CN=Bob;PARTSTAT=ACCEPTED;ROLE=OPT-PARTICIPANT:mailto:bob@example.org\r\n",
        "RRULE:FREQ=WEEKLY;COUNT=4\r\n",
        "CATEGORIES:work,daily\r\n",
        "BEGIN:VALARM\r\n",
        "ACTION:DISPLAY\r\n",
        "TRIGGER:-PT15M\r\n",
        "DESCRIPTION:Reminder\r\n",
        "END:VALARM\r\n",
        "BEGIN:VLOCATION\r\n",
        "UID:room-2\r\n",
        "NAME:Room 2\r\n",
        "END:VLOCATION\r\n",
        "END:VEVENT\r\n",
        "END:VCALENDAR\r\n",
    );

    fn event() -> GcalEvent {
        GcalEvent::from_ical(CALENDAR.as_bytes()).unwrap()
    }

    #[test]
    fn every_managed_field_survives_the_round_trip() {
        let event = event();

        assert_eq!(event.ical_uid.as_deref(), Some("event-1@example.org"));
        assert_eq!(event.summary.as_deref(), Some("Stand-up"));
        assert_eq!(event.description.as_deref(), Some("Daily"));
        assert_eq!(event.location.as_deref(), Some("Room 2"));
        assert_eq!(event.status, Some(GcalEventStatus::Confirmed));
        assert_eq!(event.transparency, Some(GcalEventTransparency::Transparent));
        assert_eq!(event.visibility, Some(GcalEventVisibility::Private));
        assert_eq!(event.sequence, Some(3));
        assert_eq!(event.recurrence, vec!["RRULE:FREQ=WEEKLY;COUNT=4"]);

        let organizer = event.organizer.as_ref().unwrap();
        assert_eq!(organizer.email.as_deref(), Some("alice@example.org"));
        assert_eq!(organizer.display_name.as_deref(), Some("Alice"));

        let attendee = &event.attendees[0];
        assert_eq!(attendee.email.as_deref(), Some("bob@example.org"));
        assert_eq!(attendee.optional, Some(true));
        assert_eq!(
            attendee.response_status,
            Some(GcalEventAttendeeResponseStatus::Accepted)
        );

        let reminders = event.reminders.as_ref().unwrap();
        assert_eq!(reminders.use_default, Some(false));
        assert_eq!(reminders.overrides[0].minutes, Some(15));
        assert_eq!(
            reminders.overrides[0].method,
            Some(GcalEventReminderMethod::Popup)
        );

        // NOTE: a recurring event needs a named zone, and the UTC
        // boundaries only carried an offset.
        assert_eq!(
            event.start.as_ref().unwrap().time_zone.as_deref(),
            Some("UTC")
        );

        let ical = event.to_ical();
        for line in [
            "UID:event-1@example.org\r\n",
            "SUMMARY:Stand-up\r\n",
            "DESCRIPTION:Daily\r\n",
            "LOCATION:Room 2\r\n",
            "STATUS:CONFIRMED\r\n",
            "TRANSP:TRANSPARENT\r\n",
            "CLASS:PRIVATE\r\n",
            "SEQUENCE:3\r\n",
            "RRULE:FREQ=WEEKLY;COUNT=4\r\n",
            "TRIGGER:-PT15M\r\n",
        ] {
            assert!(ical.contains(line), "missing {line:?} in:\n{ical}");
        }
    }

    #[test]
    fn both_boundary_shapes_project_and_a_floating_one_is_refused() {
        let all_day = CALENDAR
            .replace("DTSTART:20260814T090000Z", "DTSTART;VALUE=DATE:20260814")
            .replace("DTEND:20260814T100000Z", "DTEND;VALUE=DATE:20260815");
        let event = GcalEvent::from_ical(all_day.as_bytes()).unwrap();
        assert_eq!(
            event.start.as_ref().unwrap().date.as_deref(),
            Some("2026-08-14")
        );
        assert!(event.to_ical().contains("DTSTART;VALUE=DATE:20260814\r\n"));

        let zoned = CALENDAR.replace(
            "DTSTART:20260814T090000Z",
            "DTSTART;TZID=Europe/Paris:20260814T090000",
        );
        let event = GcalEvent::from_ical(zoned.as_bytes()).unwrap();
        let start = event.start.as_ref().unwrap();
        assert_eq!(start.date_time.as_deref(), Some("2026-08-14T09:00:00"));
        assert_eq!(start.time_zone.as_deref(), Some("Europe/Paris"));
        assert!(
            event
                .to_ical()
                .contains("DTSTART;TZID=Europe/Paris:20260814T090000\r\n")
        );

        // NOTE: no zone and no offset, so no Google form at all, and
        // the write is refused by name.
        let floating = CALENDAR.replace("DTSTART:20260814T090000Z", "DTSTART:20260814T090000");
        let err = GcalEvent::from_ical(floating.as_bytes())
            .unwrap_err()
            .to_string();
        assert!(err.contains("DTSTART"), "unexpected error: {err}");
    }

    #[test]
    fn the_remainder_is_stashed_and_spliced_back_verbatim() {
        let event = event();
        let private = &event.extended_properties.as_ref().unwrap().private;

        let stashed_event = private[&format!("{EVENT_STASH_PREFIX}0")].clone();
        assert!(stashed_event.contains("CATEGORIES:work,daily"));
        assert!(stashed_event.contains("BEGIN:VLOCATION"));

        // NOTE: the calendar-level remainder rides its own key family,
        // so a property that cannot live inside a VEVENT goes back
        // where it came from.
        let stashed_calendar = private[&format!("{CALENDAR_STASH_PREFIX}0")].clone();
        assert_eq!(stashed_calendar, "X-WR-CALNAME:Work");

        let ical = event.to_ical();
        assert!(ical.contains("CATEGORIES:work,daily\r\n"));
        assert!(ical.contains("BEGIN:VLOCATION\r\nUID:room-2\r\nNAME:Room 2\r\nEND:VLOCATION\r\n"));

        let calname = ical.find("X-WR-CALNAME:Work").unwrap();
        assert!(calname > ical.find("END:VEVENT").unwrap());
        assert!(calname < ical.find("END:VCALENDAR").unwrap());

        assert_eq!(
            GcalEvent::from_ical(ical.as_bytes())
                .unwrap()
                .extended_properties,
            event.extended_properties
        );
    }

    /// A UTC-stamped event references no zone and so owes none, while
    /// one written in Google's own web UI, a wall time under an IANA
    /// name with no stash to splice back, owes the definition.
    #[test]
    fn a_named_zone_arrives_with_the_definition_it_references() {
        assert!(!event().to_ical().contains("BEGIN:VTIMEZONE"));

        let zoned = GcalEvent {
            ical_uid: Some(String::from("google-made@google.com")),
            start: Some(GcalEventDateTime {
                date_time: Some(String::from("2024-07-14T12:00:00-04:00")),
                time_zone: Some(String::from("America/New_York")),
                ..Default::default()
            }),
            end: Some(GcalEventDateTime {
                date_time: Some(String::from("2024-07-14T13:00:00-04:00")),
                time_zone: Some(String::from("America/New_York")),
                ..Default::default()
            }),
            ..Default::default()
        };

        let ical = zoned.to_ical();
        assert!(ical.contains("DTSTART;TZID=America/New_York:20240714T120000\r\n"));
        assert!(ical.contains("TZID:America/New_York\r\n"), "{ical}");

        assert!(ical.find("BEGIN:VTIMEZONE").unwrap() < ical.find("BEGIN:VEVENT").unwrap());
        assert_eq!(ical.matches("BEGIN:VTIMEZONE").count(), 1);
    }

    /// A TZID reaches the document through more than the boundaries: an
    /// EXDATE, an RDATE or a RECURRENCE-ID rides the stash. Collecting
    /// only the zones the boundaries named would leave those dangling,
    /// their definition being no longer stashed either.
    #[test]
    fn a_zone_named_by_a_stashed_line_is_defined_too() {
        let raw = CALENDAR.replace(
            "BEGIN:VEVENT\r\n",
            concat!(
                "BEGIN:VTIMEZONE\r\n",
                "TZID:Europe/Paris\r\n",
                "BEGIN:STANDARD\r\n",
                "DTSTART:19701025T030000\r\n",
                "TZOFFSETFROM:+0200\r\n",
                "TZOFFSETTO:+0100\r\n",
                "END:STANDARD\r\n",
                "END:VTIMEZONE\r\n",
                "BEGIN:VEVENT\r\n",
                "EXDATE;TZID=Europe/Paris:20260821T110000\r\n",
            ),
        );

        let event = GcalEvent::from_ical(raw.as_bytes()).unwrap();
        let ical = event.to_ical();

        assert!(ical.contains("EXDATE;TZID=Europe/Paris:"), "{ical}");
        assert!(ical.contains("TZID:Europe/Paris\r\n"), "{ical}");
    }

    /// A folded line is one logical line split across several physical
    /// ones, so a zone name can straddle the break: reading the
    /// physical lines would see half a name and miss the reference.
    #[test]
    fn a_zone_named_across_a_fold_is_still_seen() {
        let folded = concat!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//X//EN\r\n",
            "BEGIN:VEVENT\r\nUID:folded@example.org\r\n",
            "DTSTAMP:20260101T000000Z\r\n",
            "DTSTART:20260814T090000Z\r\nDTEND:20260814T100000Z\r\n",
            "RECURRENCE-ID;TZID=Europe/\r\n Paris:20260814T110000\r\n",
            "END:VEVENT\r\nEND:VCALENDAR\r\n",
        );

        let event = GcalEvent::from_ical(folded.as_bytes()).unwrap();
        assert!(event.to_ical().contains("TZID:Europe/Paris\r\n"));
    }

    /// An IANA name the database knows costs no extended property, the
    /// projection minting its definition again on every read. A zone of
    /// its own invention could never be rebuilt, so it rides the stash.
    #[test]
    fn a_definition_is_stashed_only_when_nothing_could_rebuild_it() {
        let known = CALENDAR.replace(
            "BEGIN:VEVENT\r\n",
            concat!(
                "BEGIN:VTIMEZONE\r\n",
                "TZID:America/New_York\r\n",
                "BEGIN:STANDARD\r\n",
                "DTSTART:20071104T020000\r\n",
                "TZOFFSETFROM:-0400\r\n",
                "TZOFFSETTO:-0500\r\n",
                "END:STANDARD\r\n",
                "END:VTIMEZONE\r\n",
                "BEGIN:VEVENT\r\n",
            ),
        );

        let event = GcalEvent::from_ical(known.as_bytes()).unwrap();
        let private = &event.extended_properties.as_ref().unwrap().private;
        let stashed = private[&format!("{CALENDAR_STASH_PREFIX}0")].clone();
        assert_eq!(stashed, "X-WR-CALNAME:Work");

        let custom = known.replace("America/New_York", "Custom/Zone");
        let event = GcalEvent::from_ical(custom.as_bytes()).unwrap();
        let private = &event.extended_properties.as_ref().unwrap().private;
        let stashed = private[&format!("{CALENDAR_STASH_PREFIX}0")].clone();
        assert!(stashed.contains("TZID:Custom/Zone"), "{stashed}");
        assert!(event.to_ical().contains("TZID:Custom/Zone\r\n"));
    }

    /// An event stashed before the projection minted zones still holds
    /// a definition under a name the database knows, and rebuilding
    /// alongside it would leave two VTIMEZONE under one TZID.
    #[test]
    fn a_stashed_definition_is_not_doubled_by_a_minted_one() {
        let event = GcalEvent {
            start: Some(GcalEventDateTime {
                date_time: Some(String::from("2024-07-14T12:00:00+02:00")),
                time_zone: Some(String::from("Europe/Paris")),
                ..Default::default()
            }),
            extended_properties: Some(GcalEventExtendedProperties {
                private: BTreeMap::from([(
                    format!("{CALENDAR_STASH_PREFIX}0"),
                    String::from(concat!(
                        "BEGIN:VTIMEZONE\n",
                        "TZID:Europe/Paris\n",
                        "BEGIN:STANDARD\n",
                        "DTSTART:19701025T030000\n",
                        "TZOFFSETFROM:+0200\n",
                        "TZOFFSETTO:+0100\n",
                        "END:STANDARD\n",
                        "END:VTIMEZONE",
                    )),
                )]),
                shared: BTreeMap::new(),
            }),
            ..Default::default()
        };

        let ical = event.to_ical();
        assert_eq!(ical.matches("TZID:Europe/Paris").count(), 1, "{ical}");
    }

    #[test]
    fn a_line_too_long_for_a_chunk_stays_local_while_the_rest_is_chunked() {
        let long = format!("X-HUGE:{}", "a".repeat(MAX_STASH_CHUNK));
        let wide = format!("X-WIDE:{}", "b".repeat(MAX_STASH_CHUNK - 8));
        let calendar = CALENDAR.replace(
            "CATEGORIES:work,daily\r\n",
            &format!("{long}\r\n{wide}\r\nCATEGORIES:work,daily\r\n"),
        );

        let event = GcalEvent::from_ical(calendar.as_bytes()).unwrap();
        let private = &event.extended_properties.as_ref().unwrap().private;
        let stash: String = (0..)
            .map_while(|index| private.get(&format!("{EVENT_STASH_PREFIX}{index}")))
            .cloned()
            .collect();

        assert!(!stash.contains("X-HUGE"), "the oversized line was sent");
        assert!(stash.contains("X-WIDE"));
        assert!(stash.contains("CATEGORIES:work,daily"));

        assert!(private.len() >= 2);
        assert!(private.values().all(|chunk| chunk.len() <= MAX_STASH_CHUNK));
    }

    #[test]
    fn an_update_carries_the_display_zone_over_so_a_series_does_not_drift() {
        // NOTE: what the live API returns for a zoned recurring event:
        // the instant in UTC, the zone as a separate label. The write
        // must put the label back, or Google re-expands the series in
        // UTC and shifts every occurrence after a daylight-saving change.
        let zoned = |stamp: &str| {
            Some(GcalEventDateTime {
                date_time: Some(String::from(stamp)),
                time_zone: Some(String::from("Europe/Paris")),
                ..Default::default()
            })
        };

        let mut current = event();
        current.start = zoned("2026-10-20T07:00:00Z");
        current.end = zoned("2026-10-20T07:30:00Z");

        let merged = GcalEvent::from_ical(current.to_ical().as_bytes())
            .unwrap()
            .merge(&current);

        let start = merged.start.as_ref().unwrap();
        assert_eq!(start.date_time.as_deref(), Some("2026-10-20T07:00:00Z"));
        assert_eq!(start.time_zone.as_deref(), Some("Europe/Paris"));
        assert_eq!(
            merged.end.as_ref().unwrap().time_zone.as_deref(),
            Some("Europe/Paris")
        );
    }

    #[test]
    fn an_offset_less_boundary_never_takes_the_server_zone() {
        // NOTE: a TZID names wall time, so relabelling it would move the
        // event; only a self-describing UTC stamp may be relabelled.
        let mut current = event();
        current.start = Some(GcalEventDateTime {
            date_time: Some(String::from("2026-10-20T07:00:00Z")),
            time_zone: Some(String::from("Europe/Paris")),
            ..Default::default()
        });

        let zoned = CALENDAR.replace(
            "DTSTART:20260814T090000Z",
            "DTSTART;TZID=America/New_York:20260814T090000",
        );
        let merged = GcalEvent::from_ical(zoned.as_bytes())
            .unwrap()
            .merge(&current);

        let start = merged.start.as_ref().unwrap();
        assert_eq!(start.date_time.as_deref(), Some("2026-08-14T09:00:00"));
        assert_eq!(start.time_zone.as_deref(), Some("America/New_York"));
    }

    #[test]
    fn a_utc_stamp_keeps_its_instant_when_google_names_a_display_zone() {
        // NOTE: what the live API returns for an event written in UTC:
        // an absolute instant plus the calendar's display zone. Reading
        // the literal time as that zone's wall time would shift it.
        let mut event = event();
        event.start = Some(GcalEventDateTime {
            date_time: Some(String::from("2026-08-14T09:00:00Z")),
            time_zone: Some(String::from("Europe/Paris")),
            ..Default::default()
        });
        event.end = Some(GcalEventDateTime {
            date_time: Some(String::from("2026-08-14T10:00:00Z")),
            time_zone: Some(String::from("Europe/Paris")),
            ..Default::default()
        });

        let ical = event.to_ical();
        assert!(ical.contains("DTSTART:20260814T090000Z\r\n"), "{ical}");
        assert!(ical.contains("DTEND:20260814T100000Z\r\n"), "{ical}");
        assert!(!ical.contains("TZID"), "{ical}");
    }

    #[test]
    fn a_zoned_stamp_keeps_its_zone_so_a_series_expands_where_it_should() {
        let mut event = event();
        event.start = Some(GcalEventDateTime {
            date_time: Some(String::from("2026-08-14T09:00:00+02:00")),
            time_zone: Some(String::from("Europe/Paris")),
            ..Default::default()
        });

        let ical = event.to_ical();
        assert!(
            ical.contains("DTSTART;TZID=Europe/Paris:20260814T090000\r\n"),
            "{ical}"
        );

        // NOTE: back out offset-less, the form Google reads as wall
        // time in the named zone.
        let start = GcalEvent::from_ical(ical.as_bytes())
            .unwrap()
            .start
            .unwrap();
        assert_eq!(start.date_time.as_deref(), Some("2026-08-14T09:00:00"));
        assert_eq!(start.time_zone.as_deref(), Some("Europe/Paris"));
    }

    #[test]
    fn a_second_projection_reproduces_the_first_one_byte_for_byte() {
        let once = event().to_ical();
        let twice = GcalEvent::from_ical(once.as_bytes()).unwrap().to_ical();

        assert_eq!(once, twice);
    }

    #[test]
    fn a_non_vevent_component_is_refused_by_name() {
        let todo = concat!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\n",
            "BEGIN:VTODO\r\nUID:1\r\nDTSTAMP:20260101T000000Z\r\n",
            "SUMMARY:Not an event\r\nEND:VTODO\r\nEND:VCALENDAR\r\n",
        );

        let err = GcalEvent::from_ical(todo.as_bytes())
            .unwrap_err()
            .to_string();
        assert!(err.contains("VTODO"), "unexpected error: {err}");
    }

    #[test]
    fn a_merge_keeps_the_provider_only_fields_and_clears_what_the_document_dropped() {
        let mut current = event();
        current.color_id = Some(String::from("7"));
        current.guests_can_modify = Some(true);
        current.summary = Some(String::from("Old title"));
        current
            .extended_properties
            .as_mut()
            .unwrap()
            .private
            .insert(String::from("other-client.key"), String::from("keep me"));

        let stripped = CALENDAR.replace("LOCATION:Room 2\r\n", "");
        let merged = GcalEvent::from_ical(stripped.as_bytes())
            .unwrap()
            .merge(&current);

        assert_eq!(merged.color_id.as_deref(), Some("7"));
        assert_eq!(merged.guests_can_modify, Some(true));

        assert_eq!(merged.summary.as_deref(), Some("Stand-up"));
        assert_eq!(merged.location, None);

        let private = &merged.extended_properties.as_ref().unwrap().private;
        assert_eq!(
            private.get("other-client.key").map(String::as_str),
            Some("keep me")
        );
        assert!(private.contains_key(&format!("{EVENT_STASH_PREFIX}0")));
    }

    /// A Meet with a video and a phone entry point.
    fn meet() -> GcalConferenceData {
        GcalConferenceData {
            entry_points: vec![
                GcalEntryPoint {
                    entry_point_type: Some(GcalEntryPointType::Video),
                    uri: Some(String::from("https://meet.example.org/abc-defg-hij")),
                    label: Some(String::from("meet.example.org/abc-defg-hij")),
                    ..GcalEntryPoint::default()
                },
                GcalEntryPoint {
                    entry_point_type: Some(GcalEntryPointType::Phone),
                    uri: Some(String::from("tel:+33-1-00-00-00-00")),
                    label: Some(String::from("+33 1 00 00 00 00")),
                    pin: Some(String::from("123456")),
                    ..GcalEntryPoint::default()
                },
            ],
            ..GcalConferenceData::default()
        }
    }

    #[test]
    fn each_entry_point_is_minted_as_a_conference_and_dropped_on_the_way_back() {
        let mut event = event();
        event.conference_data = Some(meet());

        let document = event.to_ical();

        assert!(
            document.contains(
                "CONFERENCE;VALUE=URI;FEATURE=AUDIO,VIDEO;LABEL=meet.example.org/abc-defg-hij:https://meet.example.org/abc-defg-hij\r\n"
            ),
            "{document}"
        );
        assert!(
            document.contains("CONFERENCE;VALUE=URI;FEATURE=PHONE;LABEL=+33 1 00 00 00 00:tel:+33-1-00-00-00-00\r\n"),
            "{document}"
        );
        assert!(!document.contains("X-GOOGLE-CONFERENCE"), "{document}");

        let back = GcalEvent::from_ical(document.as_bytes()).unwrap();
        assert_eq!(back.conference_data, None);
        assert!(!format!("{:?}", back.extended_properties).contains("CONFERENCE"));
    }

    #[test]
    fn a_conference_minted_under_its_former_name_is_dropped_too() {
        let legacy = CALENDAR.replace(
            "SUMMARY:Stand-up\r\n",
            "SUMMARY:Stand-up\r\nX-GOOGLE-CONFERENCE:https://meet.example.org/x\r\n",
        );
        let event = GcalEvent::from_ical(legacy.as_bytes()).unwrap();
        assert!(!format!("{:?}", event.extended_properties).contains("X-GOOGLE-CONFERENCE"));
    }

    #[test]
    fn an_online_meeting_asked_for_becomes_a_stable_meet_request() {
        let asked = CALENDAR.replace(
            "SUMMARY:Stand-up\r\n",
            "SUMMARY:Stand-up\r\nX-PIMDIR-ONLINE-MEETING:TRUE\r\n",
        );
        let event = GcalEvent::from_ical(asked.as_bytes()).unwrap();

        let request = event
            .conference_data
            .as_ref()
            .and_then(|conference| conference.create_request.as_ref())
            .expect("a create request");
        assert_eq!(
            request
                .conference_solution_key
                .as_ref()
                .and_then(|key| key.solution_type),
            Some(GcalConferenceSolutionType::HangoutsMeet)
        );
        let id = request.request_id.clone().unwrap();
        assert!(id.starts_with("pimdir-"), "{id}");
        assert!(!format!("{:?}", event.extended_properties).contains("ONLINE-MEETING"));

        // NOTE: the same document asks again with the same id, so Google
        // replays the meeting instead of minting a second one.
        let again = GcalEvent::from_ical(asked.as_bytes()).unwrap();
        assert_eq!(
            again
                .conference_data
                .unwrap()
                .create_request
                .unwrap()
                .request_id,
            Some(id.clone())
        );

        let restamped = asked.replace("DTSTAMP:20260101T000000Z", "DTSTAMP:20260102T000000Z");
        let other = GcalEvent::from_ical(restamped.as_bytes()).unwrap();
        assert_ne!(
            other
                .conference_data
                .unwrap()
                .create_request
                .unwrap()
                .request_id,
            Some(id)
        );

        let declined = CALENDAR.replace(
            "SUMMARY:Stand-up\r\n",
            "SUMMARY:Stand-up\r\nX-PIMDIR-ONLINE-MEETING:FALSE\r\n",
        );
        let event = GcalEvent::from_ical(declined.as_bytes()).unwrap();
        assert_eq!(event.conference_data, None);
        assert!(!format!("{:?}", event.extended_properties).contains("ONLINE-MEETING"));
    }

    #[test]
    fn a_merge_keeps_the_conference_the_server_holds() {
        let mut current = event();
        current.conference_data = Some(meet());

        let merged = event().merge(&current);
        assert_eq!(merged.conference_data, Some(meet()));

        // NOTE: asking again for a meeting the event already has creates
        // none (Annex B.1: only a component the current one lacks).
        let asked = CALENDAR.replace(
            "SUMMARY:Stand-up\r\n",
            "SUMMARY:Stand-up\r\nX-PIMDIR-ONLINE-MEETING:TRUE\r\n",
        );
        let merged = GcalEvent::from_ical(asked.as_bytes())
            .unwrap()
            .merge(&current);
        assert_eq!(merged.conference_data, Some(meet()));

        let merged = GcalEvent::from_ical(asked.as_bytes())
            .unwrap()
            .merge(&event());
        assert!(merged.conference_data.unwrap().create_request.is_some());
    }

    #[test]
    fn an_alarm_google_cannot_model_stays_in_the_stash() {
        let absolute =
            CALENDAR.replace("TRIGGER:-PT15M", "TRIGGER;VALUE=DATE-TIME:20260814T084500Z");
        let event = GcalEvent::from_ical(absolute.as_bytes()).unwrap();

        assert_eq!(event.reminders.as_ref().unwrap().use_default, Some(true));
        assert!(event.reminders.as_ref().unwrap().overrides.is_empty());

        let stash = event.to_ical();
        assert!(stash.contains("TRIGGER;VALUE=DATE-TIME:20260814T084500Z\r\n"));
    }

    #[test]
    fn a_lead_time_reads_only_the_negative_whole_minute_durations() {
        assert_eq!(lead_minutes("-PT15M"), Some(15));
        assert_eq!(lead_minutes("-P1D"), Some(1440));
        assert_eq!(lead_minutes("-P1DT2H30M"), Some(1590));
        assert_eq!(lead_minutes("-PT0M"), Some(0));

        assert_eq!(lead_minutes("PT15M"), None);
        assert_eq!(lead_minutes("-PT90S"), None);
        assert_eq!(lead_minutes("-P5W"), None);
        assert_eq!(lead_minutes("-PT15X"), None);
        assert_eq!(lead_minutes("-PTM"), None);
    }

    #[test]
    fn an_exception_names_the_instance_it_replaces() {
        let mut exception = event();
        exception.recurrence.clear();
        exception.recurring_event_id = Some(String::from("master-1"));
        exception.original_start_time = Some(GcalEventDateTime {
            date_time: Some(String::from("2026-08-11T09:00:00Z")),
            time_zone: Some(String::from("UTC")),
            ..Default::default()
        });

        let ical = exception.to_ical();

        assert!(
            ical.contains("RECURRENCE-ID:20260811T090000Z\r\n"),
            "{ical}"
        );
    }

    #[test]
    fn a_series_and_its_exception_fold_into_one_resource() {
        let mut master = event();
        master.id = Some(String::from("master-1"));

        let mut exception = event();
        exception.id = Some(String::from("master-1_20260811T090000Z"));
        exception.ical_uid = Some(String::from("minted-by-google@google.com"));
        exception.recurrence.clear();
        exception.summary = Some(String::from("Stand-up moved"));
        exception.recurring_event_id = Some(String::from("master-1"));
        exception.original_start_time = Some(GcalEventDateTime {
            date_time: Some(String::from("2026-08-11T09:00:00Z")),
            time_zone: Some(String::from("UTC")),
            ..Default::default()
        });

        let document = master.to_ical_series(&[&exception]);

        // NOTE: one resource, two components, and the one UID RFC 4791
        // 4.1 allows a resource to carry.
        assert_eq!(document.matches("BEGIN:VEVENT\r\n").count(), 2);
        assert_eq!(document.matches("BEGIN:VCALENDAR\r\n").count(), 1);
        assert_eq!(
            document.matches("UID:event-1@example.org\r\n").count(),
            2,
            "{document}"
        );
        assert!(!document.contains("minted-by-google@google.com"));

        assert!(document.contains("RRULE:FREQ=WEEKLY;COUNT=4\r\n"));
        assert!(document.contains("RECURRENCE-ID:20260811T090000Z\r\n"));
        assert!(document.contains("SUMMARY:Stand-up moved\r\n"));

        // NOTE: a whole component, not a run of lines.
        assert_eq!(document.matches("END:VEVENT\r\n").count(), 2);
    }

    #[test]
    fn a_folded_exception_gets_the_zone_it_names_defined() {
        let mut master = event();
        master.id = Some(String::from("master-1"));

        let mut exception = event();
        exception.recurrence.clear();
        exception.recurring_event_id = Some(String::from("master-1"));
        exception.original_start_time = Some(GcalEventDateTime {
            date_time: Some(String::from("2026-08-11T09:00:00Z")),
            time_zone: Some(String::from("UTC")),
            ..Default::default()
        });
        exception.start = Some(GcalEventDateTime {
            date_time: Some(String::from("2026-08-11T14:00:00+02:00")),
            time_zone: Some(String::from("Europe/Paris")),
            ..Default::default()
        });

        let document = master.to_ical_series(&[&exception]);

        // NOTE: RFC 5545 3.2.19, so a TZID the master never named still
        // owes a VTIMEZONE minted over the folded document.
        assert!(document.contains("TZID=Europe/Paris"), "{document}");
        assert!(document.contains("TZID:Europe/Paris\r\n"), "{document}");
    }
}
