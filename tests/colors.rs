//! Offline coverage of the colour palettes (`colors`).

mod common;

use common::*;
use io_gcal::v3::rest::colors::{GcalColors, get::GcalColorsGet};

const COLORS: &str = r##"{"kind":"calendar#colors","updated":"2012-02-14T00:00:00.000Z","calendar":{"1":{"background":"#ac725e","foreground":"#1d1d1d"}},"event":{"1":{"background":"#a4bdfc","foreground":"#1d1d1d"},"2":{"background":"#7ae7bf","foreground":"#1d1d1d"}}}"##;

#[test]
fn gets_the_palettes() {
    let mut coroutine = GcalColorsGet::new(&auth()).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", COLORS));
    let out = ret.unwrap();

    assert_eq!(out.response.kind.as_deref(), Some("calendar#colors"));
    assert_eq!(out.response.event.len(), 2);
    assert_eq!(
        out.response.event["1"].background.as_deref(),
        Some("#a4bdfc")
    );
    assert_eq!(
        out.response.calendar["1"].foreground.as_deref(),
        Some("#1d1d1d")
    );

    assert_eq!(request_line(&request), "GET /calendar/v3/colors");
}

#[test]
fn parses_empty_palettes() {
    let colors: GcalColors = serde_json::from_str("{}").unwrap();

    assert!(colors.event.is_empty());
    assert!(colors.calendar.is_empty());
}
