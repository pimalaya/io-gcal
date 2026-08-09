//! Offline coverage of the query serializer: what reaches the URL, and
//! what deliberately does not.

mod common;

use std::collections::BTreeMap;

use io_gcal::v3::query::{GcalQueryError, append_query_pairs, is_false, to_query_pairs};
use serde::{Serialize, ser::Error as SerError};
use url::Url;

type Map = BTreeMap<String, u32>;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Params<'a> {
    text: Option<&'a str>,
    #[serde(rename = "iCalUID")]
    ical_uid: Option<&'a str>,
    count: Option<u32>,
    ratio: Option<f32>,
    letter: Option<char>,
    ordering: Option<Ordering>,
    tags: &'a [&'a str],
    #[serde(skip_serializing_if = "is_false")]
    flag: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
enum Ordering {
    StartTime,
}

#[derive(Debug, Default, Serialize)]
struct Nested {
    inner: Inner,
}

#[derive(Debug, Default, Serialize)]
struct Inner {
    value: u32,
}

#[derive(Debug, Serialize)]
struct One<T> {
    field: T,
}

#[derive(Debug, Serialize)]
struct UnitStruct;

#[derive(Debug, Serialize)]
struct NewtypeStruct(u32);

#[derive(Debug, Serialize)]
struct NewtypeParams(One<u32>);

#[derive(Debug, Serialize)]
struct TupleStruct(u32, u32);

#[derive(Debug, Serialize)]
enum Shape {
    Unit,
    Newtype(u32),
    Tuple(u32, u32),
    Struct { value: u32 },
}

#[derive(Debug, Serialize)]
struct Widths {
    small: i8,
    short: i16,
    int: i32,
    long: i64,
    byte: u8,
    ushort: u16,
    uint: u32,
    ulong: u64,
    float: f32,
    double: f64,
}

/// A value that insists on serializing as raw bytes, which no query
/// parameter ever is.
#[derive(Debug)]
struct Bytes;

impl Serialize for Bytes {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(b"raw")
    }
}

/// Asserts that a value cannot be flattened into query pairs at all.
fn rejects<T: Serialize>(value: T) {
    let pairs = to_query_pairs(&value);
    assert!(pairs.is_empty(), "got: {pairs:?}");
}

/// Asserts that a value cannot be flattened as the field of a params
/// struct, taking the whole struct down with it.
fn rejects_as_field<T: Serialize>(value: T) {
    rejects(One { field: value });
}

#[test]
fn flattens_every_supported_shape() {
    let params = Params {
        text: Some("hello"),
        ical_uid: Some("uid@example.org"),
        count: Some(7),
        ratio: Some(0.5),
        letter: Some('x'),
        ordering: Some(Ordering::StartTime),
        tags: &["a", "b"],
        flag: true,
    };

    let pairs = to_query_pairs(&params);

    assert_eq!(
        pairs,
        vec![
            (String::from("text"), String::from("hello")),
            (String::from("iCalUID"), String::from("uid@example.org")),
            (String::from("count"), String::from("7")),
            (String::from("ratio"), String::from("0.5")),
            (String::from("letter"), String::from("x")),
            (String::from("ordering"), String::from("startTime")),
            (String::from("tags"), String::from("a")),
            (String::from("tags"), String::from("b")),
            (String::from("flag"), String::from("true")),
        ]
    );
}

#[test]
fn drops_what_the_caller_left_unset() {
    let pairs = to_query_pairs(&Params::default());

    assert!(pairs.is_empty(), "got: {pairs:?}");
}

#[test]
fn repeats_the_key_of_a_sequence() {
    let params = Params {
        tags: &["one", "two", "three"],
        ..Default::default()
    };

    let pairs = to_query_pairs(&params);

    assert_eq!(pairs.len(), 3);
    assert!(pairs.iter().all(|(key, _)| key == "tags"));
}

#[test]
fn refuses_anything_that_is_not_a_struct() {
    // NOTE: the serializer errors on these, and `to_query_pairs`
    // swallows the error into an empty list, so a params struct that
    // grows a nested field silently stops reaching the wire. The
    // per-method tests asserting on the request line are what catch
    // that.
    rejects(true);
    rejects(1i8);
    rejects(1i16);
    rejects(1i32);
    rejects(1i64);
    rejects(1u8);
    rejects(1u16);
    rejects(1u32);
    rejects(1u64);
    rejects(1.5f32);
    rejects(1.5f64);
    rejects('x');
    rejects("bare");
    rejects(Option::<u32>::None);
    rejects(Some(1u32));
    rejects(());
    rejects(UnitStruct);
    rejects(Shape::Unit);
    rejects(Shape::Newtype(1));
    rejects(vec![1u32]);
    rejects((1u32, 2u32));
    rejects(TupleStruct(1, 2));
    rejects(Shape::Tuple(1, 2));
    rejects([(String::from("a"), 1u32)].into_iter().collect::<Map>());
    rejects(Shape::Struct { value: 1 });
    rejects(Bytes);
}

#[test]
fn stringifies_every_number_width() {
    let widths = Widths {
        small: -1,
        short: -2,
        int: -3,
        long: -4,
        byte: 1,
        ushort: 2,
        uint: 3,
        ulong: 4,
        float: 0.5,
        double: 1.25,
    };

    let pairs = to_query_pairs(&widths);
    let values: Vec<&str> = pairs.iter().map(|(_, value)| value.as_str()).collect();

    assert_eq!(
        values,
        vec!["-1", "-2", "-3", "-4", "1", "2", "3", "4", "0.5", "1.25"]
    );
}

#[test]
fn refuses_a_field_that_is_not_a_scalar_or_a_sequence() {
    rejects_as_field(());
    rejects_as_field(UnitStruct);
    rejects_as_field(Shape::Newtype(1));
    rejects_as_field((1u32, 2u32));
    rejects_as_field(TupleStruct(1, 2));
    rejects_as_field(Shape::Tuple(1, 2));
    rejects_as_field([(String::from("a"), 1u32)].into_iter().collect::<Map>());
    rejects_as_field(Inner::default());
    rejects_as_field(Shape::Struct { value: 1 });
    rejects_as_field(Bytes);
    rejects(Nested::default());
}

#[test]
fn names_what_it_refused() {
    let err = GcalQueryError::custom("boom");

    assert_eq!(err.to_string(), "boom");
}

#[test]
fn unwraps_the_newtypes_on_the_way_to_the_wire() {
    // NOTE: a newtype is transparent on both sides: wrapping the whole
    // params struct, and wrapping one of its fields.
    let pairs = to_query_pairs(&One {
        field: NewtypeStruct(7),
    });
    assert_eq!(pairs, vec![(String::from("field"), String::from("7"))]);

    let pairs = to_query_pairs(&NewtypeParams(One { field: 7u32 }));
    assert_eq!(pairs, vec![(String::from("field"), String::from("7"))]);
}

#[test]
fn appends_nothing_when_there_is_nothing_to_append() {
    let mut url = Url::parse("https://www.googleapis.com/calendar/v3/colors").unwrap();

    append_query_pairs(&mut url, &Params::default());

    // NOTE: not `.../colors?`, which is what going through
    // `query_pairs_mut` unconditionally would produce.
    assert_eq!(
        url.as_str(),
        "https://www.googleapis.com/calendar/v3/colors"
    );
    assert!(url.query().is_none());
}

#[test]
fn appends_the_pairs_percent_encoded() {
    let mut url = Url::parse("https://www.googleapis.com/calendar/v3/events").unwrap();
    let params = Params {
        text: Some("a b&c=d"),
        tags: &["x/y"],
        ..Default::default()
    };

    append_query_pairs(&mut url, &params);

    assert_eq!(
        url.as_str(),
        "https://www.googleapis.com/calendar/v3/events?text=a+b%26c%3Dd&tags=x%2Fy"
    );
}

#[test]
fn keeps_the_pairs_a_url_already_carries() {
    let mut url = Url::parse("https://www.googleapis.com/calendar/v3/events?alt=json").unwrap();
    let params = Params {
        count: Some(1),
        ..Default::default()
    };

    append_query_pairs(&mut url, &params);

    assert_eq!(url.query(), Some("alt=json&count=1"));
}

#[test]
fn only_skips_a_false_flag() {
    assert!(is_false(&false));
    assert!(!is_false(&true));
}
