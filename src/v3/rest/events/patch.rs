//! Patch an event (`events.patch`).
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference/events/patch>

use alloc::format;

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use serde::Serialize;
use url::Url;

use crate::{
    coroutine::*,
    gcal_try,
    v3::{
        query::{append_query_pairs, is_false},
        rest::events::{GcalEvent, GcalSendUpdates},
        send::{GCAL_API_BASE, GcalSend, GcalSendError, GcalSendOutput},
    },
};

/// Query parameters for patching an event (`events.patch`).
#[derive(Debug, Clone, Default, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GcalEventPatchParams {
    /// Who gets notified of the change by email.
    pub send_updates: Option<GcalSendUpdates>,
    /// The version of the conference data the client supports, `0` for
    /// none and `1` for the create requests it can honour. Conference
    /// data on the event is ignored when it is left unset.
    pub conference_data_version: Option<u8>,
    /// The version of the event labels the client supports; the event
    /// label id is only honoured when it is `1`.
    pub event_label_version: Option<u8>,
    /// The maximum number of attendees to return on the patched event.
    pub max_attendees: Option<u32>,
    /// Whether the client supports attachments; they are dropped
    /// otherwise.
    #[serde(skip_serializing_if = "is_false")]
    pub supports_attachments: bool,
}

/// I/O-free coroutine patching an event (`events.patch`).
pub struct GcalEventPatch {
    send: GcalSend<GcalEvent>,
}

impl GcalEventPatch {
    /// Builds the `events.patch` request, merging only the fields the
    /// given event sets into the event of the given id.
    ///
    /// `if_match` gates the write on the etag a read returned, so a
    /// concurrent change comes back as HTTP 412 rather than being
    /// merged into; `None` patches unconditionally.
    pub fn new(
        auth: &HttpAuthBearer,
        calendar_id: &str,
        event_id: &str,
        event: &GcalEvent,
        params: &GcalEventPatchParams,
        if_match: Option<&str>,
    ) -> Result<Self, GcalSendError> {
        debug!("prepare calendar event for patch");
        trace!("calendar_id: {calendar_id:?}");
        trace!("event_id: {event_id:?}");
        trace!("event: {event:?}");
        trace!("params: {params:?}");

        let mut url = Url::parse(GCAL_API_BASE)?
            .join(&format!("calendars/{calendar_id}/events/{event_id}"))?;
        append_query_pairs(&mut url, params);

        let send = GcalSend::patch_json_if_match(auth, url, event, if_match)?;

        Ok(Self { send })
    }
}

impl GcalCoroutine for GcalEventPatch {
    type Yield = GcalYield;
    type Return = Result<GcalSendOutput<GcalEvent>, GcalSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GcalCoroutineState<Self::Yield, Self::Return> {
        let out = gcal_try!(&mut self.send, arg);
        debug!("event patched");
        trace!("out: {out:?}");
        GcalCoroutineState::Complete(Ok(out))
    }
}
