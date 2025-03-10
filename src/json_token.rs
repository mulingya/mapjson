use std::fmt::Debug;
use std::hash::Hash;

#[derive(Debug, PartialEq, Hash)]
pub enum JsonToken {
    Null,
    False,
    True,
    StringValue(String),
    Number(String),
    Name(String),
    StartObject,
    EndObject,
    StartArray,
    EndArray,
    EndDocument,
}
