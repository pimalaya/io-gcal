//! Import an event into a calendar (`events.import`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/events/import>

use alloc::format;

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use serde::Serialize;
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        query::{is_false, to_query_pairs},
        rest::events::GcalEvent,
        send::{GCAL_API_BASE, GcalSend, GcalSendError, GcalSendOutput},
    },
};

/// Query parameters for importing an event (`events.import`).
#[derive(Debug, Clone, Default, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalEventImportParams {
    /// The version of the conference data the client supports, `0` for
    /// none and `1` for the create requests it can honour. Conference
    /// data on the event is ignored when it is left unset.
    pub conference_data_version: Option<u8>,
    /// The version of the event labels the client supports; the event
    /// label id is only honoured when it is `1`.
    pub event_label_version: Option<u8>,
    /// Whether the client supports attachments; they are dropped
    /// otherwise.
    #[serde(skip_serializing_if = "is_false")]
    pub supports_attachments: bool,
}

/// I/O-free coroutine importing an existing event into a calendar
/// (`events.import`).
pub struct GcalEventImport {
    send: GcalSend<GcalEvent>,
}

impl GcalEventImport {
    /// Builds the `events.import` request from the given event.
    ///
    /// Importing adds a private copy of an event that already exists
    /// elsewhere, so the event must carry its RFC 5545 unique
    /// identifier along with its start and end. Only an event whose
    /// type is [`crate::v3::rest::events::GcalEventType::Default`] can
    /// be imported.
    pub fn new(
        auth: &HttpAuthBearer,
        calendar_id: &str,
        event: &GcalEvent,
        params: &GcalEventImportParams,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar event for import");
        trace!("calendar_id: {calendar_id:?}");
        trace!("event: {event:?}");
        trace!("params: {params:?}");

        let ical_uid_is_empty = event
            .ical_uid
            .as_ref()
            .is_none_or(|ical_uid| ical_uid.trim().is_empty());

        if ical_uid_is_empty {
            let err = GcalSendError::InvalidRequest("Imported event requires an iCalUID".into());
            return Err(err);
        }

        if event.start.is_none() || event.end.is_none() {
            let err = GcalSendError::InvalidRequest("Event start and end are required".into());
            return Err(err);
        }

        let mut url =
            Url::parse(GCAL_API_BASE)?.join(&format!("calendars/{calendar_id}/events/import"))?;
        url.query_pairs_mut().extend_pairs(to_query_pairs(params));

        let send = GcalSend::post_json(auth, url, event)?;

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalEventImport {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalEvent>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("event imported");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
