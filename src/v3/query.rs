//! Turn a `Serialize` struct into URL query pairs.
//!
//! A minimal `no_std` serde serializer that flattens a flat struct of
//! scalars, options and sequences into `(key, value)` string pairs ready
//! for `url`'s `query_pairs_mut().extend_pairs(...)`. Keys come from the
//! struct field names (so a `#[serde(rename_all = "camelCase")]` params
//! struct yields the Calendar query keys for free); `None` and empty
//! sequences produce nothing, and a sequence produces one repeated-key
//! pair per element.

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::fmt;

use serde::{
    Serialize, Serializer,
    ser::{Error as SerError, Impossible, SerializeSeq, SerializeStruct},
};
use url::Url;

/// Appends the query pairs of `params` to `url`.
///
/// A params struct that serializes to nothing leaves the URL
/// untouched: going through `query_pairs_mut` unconditionally would
/// append a bare `?` to every parameterless request.
pub fn append_query_pairs<T>(url: &mut Url, params: &T)
where
    T: Serialize + ?Sized,
{
    let pairs = to_query_pairs(params);

    if !pairs.is_empty() {
        url.query_pairs_mut().extend_pairs(pairs);
    }
}

/// Serialize `value` into a list of URL query `(key, value)` pairs.
pub fn to_query_pairs<T>(value: &T) -> Vec<(String, String)>
where
    T: Serialize + ?Sized,
{
    value.serialize(QuerySerializer).unwrap_or_default()
}

/// Serialize a single field `value` into URL query pairs under `key`.
///
/// Same rules as a struct field of [`to_query_pairs`]: `None` produces
/// nothing and a unit enum variant produces its serde-renamed name.
pub fn to_field_pairs<T>(key: &'static str, value: &T) -> Vec<(String, String)>
where
    T: Serialize + ?Sized,
{
    let mut pairs = Vec::new();
    let field = FieldQuery {
        key,
        pairs: &mut pairs,
    };

    match value.serialize(field) {
        Ok(()) => pairs,
        Err(_) => Vec::new(),
    }
}

/// Predicate for `#[serde(skip_serializing_if = ...)]` on bool flags that
/// should only appear in the query when set.
pub fn is_false(value: &bool) -> bool {
    !*value
}

/// Error raised when a value cannot be flattened into query pairs.
#[derive(Debug)]
pub struct GcalQueryError(String);

impl fmt::Display for GcalQueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for GcalQueryError {}

impl SerError for GcalQueryError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self(msg.to_string())
    }
}

fn unsupported() -> GcalQueryError {
    GcalQueryError(String::from(
        "Query parameters must be a flat struct of scalars and sequences",
    ))
}

/// Top-level serializer: only a struct maps to query pairs.
struct QuerySerializer;

impl Serializer for QuerySerializer {
    type Ok = Vec<(String, String)>;
    type Error = GcalQueryError;
    type SerializeSeq = Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = Impossible<Self::Ok, Self::Error>;
    type SerializeMap = Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = StructQuery;
    type SerializeStructVariant = Impossible<Self::Ok, Self::Error>;

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(StructQuery {
            pairs: Vec::with_capacity(len),
        })
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }

    fn serialize_bool(self, _: bool) -> Result<Self::Ok, Self::Error> {
        Err(unsupported())
    }
    fn serialize_i8(self, _: i8) -> Result<Self::Ok, Self::Error> {
        Err(unsupported())
    }
    fn serialize_i16(self, _: i16) -> Result<Self::Ok, Self::Error> {
        Err(unsupported())
    }
    fn serialize_i32(self, _: i32) -> Result<Self::Ok, Self::Error> {
        Err(unsupported())
    }
    fn serialize_i64(self, _: i64) -> Result<Self::Ok, Self::Error> {
        Err(unsupported())
    }
    fn serialize_u8(self, _: u8) -> Result<Self::Ok, Self::Error> {
        Err(unsupported())
    }
    fn serialize_u16(self, _: u16) -> Result<Self::Ok, Self::Error> {
        Err(unsupported())
    }
    fn serialize_u32(self, _: u32) -> Result<Self::Ok, Self::Error> {
        Err(unsupported())
    }
    fn serialize_u64(self, _: u64) -> Result<Self::Ok, Self::Error> {
        Err(unsupported())
    }
    fn serialize_f32(self, _: f32) -> Result<Self::Ok, Self::Error> {
        Err(unsupported())
    }
    fn serialize_f64(self, _: f64) -> Result<Self::Ok, Self::Error> {
        Err(unsupported())
    }
    fn serialize_char(self, _: char) -> Result<Self::Ok, Self::Error> {
        Err(unsupported())
    }
    fn serialize_str(self, _: &str) -> Result<Self::Ok, Self::Error> {
        Err(unsupported())
    }
    fn serialize_bytes(self, _: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(unsupported())
    }
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(unsupported())
    }
    fn serialize_some<T>(self, _: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        Err(unsupported())
    }
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(unsupported())
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(unsupported())
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(unsupported())
    }
    fn serialize_newtype_variant<T>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        Err(unsupported())
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(unsupported())
    }
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(unsupported())
    }
    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(unsupported())
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(unsupported())
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(unsupported())
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(unsupported())
    }
}

/// Collects one field at a time into query pairs.
struct StructQuery {
    pairs: Vec<(String, String)>,
}

impl SerializeStruct for StructQuery {
    type Ok = Vec<(String, String)>;
    type Error = GcalQueryError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(FieldQuery {
            key,
            pairs: &mut self.pairs,
        })
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(self.pairs)
    }
}

/// Serializes a single field value under a fixed key, pushing one pair
/// per scalar and one per sequence element (skipping `None`).
struct FieldQuery<'a> {
    key: &'static str,
    pairs: &'a mut Vec<(String, String)>,
}

impl FieldQuery<'_> {
    fn push(self, value: String) -> Result<(), GcalQueryError> {
        self.pairs.push((self.key.to_string(), value));
        Ok(())
    }
}

impl<'a> Serializer for FieldQuery<'a> {
    type Ok = ();
    type Error = GcalQueryError;
    type SerializeSeq = SeqQuery<'a>;
    type SerializeTuple = Impossible<(), GcalQueryError>;
    type SerializeTupleStruct = Impossible<(), GcalQueryError>;
    type SerializeTupleVariant = Impossible<(), GcalQueryError>;
    type SerializeMap = Impossible<(), GcalQueryError>;
    type SerializeStruct = Impossible<(), GcalQueryError>;
    type SerializeStructVariant = Impossible<(), GcalQueryError>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        self.push(v.to_string())
    }
    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.push(v.to_string())
    }
    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.push(v.to_string())
    }
    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.push(v.to_string())
    }
    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.push(v.to_string())
    }
    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.push(v.to_string())
    }
    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.push(v.to_string())
    }
    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.push(v.to_string())
    }
    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.push(v.to_string())
    }
    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.push(v.to_string())
    }
    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.push(v.to_string())
    }
    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        self.push(v.to_string())
    }
    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.push(v.to_string())
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.push(variant.to_string())
    }
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }
    fn serialize_newtype_struct<T>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(SeqQuery {
            key: self.key,
            pairs: self.pairs,
        })
    }

    fn serialize_bytes(self, _: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(unsupported())
    }
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(unsupported())
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(unsupported())
    }
    fn serialize_newtype_variant<T>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        Err(unsupported())
    }
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(unsupported())
    }
    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(unsupported())
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(unsupported())
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(unsupported())
    }
    fn serialize_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(unsupported())
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(unsupported())
    }
}

/// Serializes each sequence element as a repeated-key pair.
struct SeqQuery<'a> {
    key: &'static str,
    pairs: &'a mut Vec<(String, String)>,
}

impl SerializeSeq for SeqQuery<'_> {
    type Ok = ();
    type Error = GcalQueryError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(FieldQuery {
            key: self.key,
            pairs: self.pairs,
        })
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}
