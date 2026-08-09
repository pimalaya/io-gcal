//! Full std client: list the upcoming events of the primary calendar.
//!
//! Opens the TCP and TLS connection to the Calendar API, lists the
//! events of the primary calendar with recurring series expanded into
//! instances, and prints each one. Needs an OAuth 2.0 access token with
//! the calendar.readonly scope, read from the `GCAL_ACCESS_TOKEN`
//! environment variable:
//!
//! ```sh
//! GCAL_ACCESS_TOKEN="<token>" cargo run --example std_gcal
//! ```

use std::env;

use io_gcal::v3::{client::GcalClientStd, rest::events::list::GcalEventsListParams};

fn main() {
    env_logger::try_init().ok();

    let token = env::var("GCAL_ACCESS_TOKEN").expect("GCAL_ACCESS_TOKEN not set");

    let mut client = GcalClientStd::connect(token, Default::default()).unwrap();

    let params = GcalEventsListParams {
        max_results: Some(10),
        single_events: true,
        ..Default::default()
    };

    let out = client.events_list("primary", &params).unwrap();

    for event in &out.response.items {
        let start = event
            .start
            .as_ref()
            .and_then(|start| start.date_time.as_deref().or(start.date.as_deref()))
            .unwrap_or("(no start)");

        println!(
            "{start}: {}",
            event.summary.as_deref().unwrap_or("(no title)")
        );
    }
}
