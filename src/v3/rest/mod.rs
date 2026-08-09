//! Google Calendar REST API, mirroring the reference tree: each
//! resource is a module and each method a file named after the API
//! method in snake_case.
//!
//! <https://developers.google.com/workspace/calendar/api/v3/reference>

pub mod acl;
pub mod calendar_list;
pub mod calendars;
pub mod channels;
pub mod colors;
pub mod events;
pub mod freebusy;
pub mod settings;
