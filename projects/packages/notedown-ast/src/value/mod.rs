#![doc = include_str!("readme.md")]

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{Debug, Formatter},
};

use diagnostic_quick::error_3rd::NodeLocation;
use indexmap::IndexMap;
use num::BigInt;
use rust_decimal::Decimal;

use crate::{value::list::List, Dict};

pub use self::typing::NotedownType;

pub mod dict;
pub mod list;
mod methods;
mod traits;
mod typing;

///
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NotedownValue {
    /// It doesn't look like anything to me
    Null,
    /// `true` or `false`
    Boolean(bool),
    /// Arbitrarily large integer
    Integer(BigInt),
    /// 128-bit fixed point decimal, enough for financial calculations
    Decimal(Decimal),
    /// A UTF-8–encoded string
    String(String),
    /// Array of values
    Array(List<NotedownValue>),
    /// Ordered map of key value pairs
    Object(Dict<NotedownValue>),
}

impl Default for NotedownValue {
    fn default() -> Self {
        Self::Null
    }
}

impl NotedownValue {
    /// convert a integer to value
    pub fn integer(value: impl Into<BigInt>) -> Self {
        Self::Integer(value.into())
    }
    pub fn as_integer(&self) -> Option<&BigInt> {
        match self {
            Self::Integer(value) => Some(value),
            _ => None,
        }
    }
    /// convert a decimal to value
    pub fn decimal(value: impl Into<Decimal>) -> Self {
        Self::Decimal(value.into())
    }
    pub fn as_decimal(&self) -> Option<&Decimal> {
        match self {
            Self::Decimal(value) => Some(value),
            _ => None,
        }
    }
    /// convert a string to value
    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }
    pub fn as_string(&self) -> Option<&String> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }
}
