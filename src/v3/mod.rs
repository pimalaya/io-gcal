//! Google Calendar API v3.
//!
//! `rest` mirrors the REST resource tree (`acl`, `calendarList`,
//! `calendars`, `channels`, `colors`, `events`, `freebusy`,
//! `settings`); the sibling modules hold the transport primitive, the
//! query serializer and the optional std client.

#[cfg(feature = "client")]
pub mod client;
pub mod query;
pub mod rest;
pub mod send;
