use chrono::{NaiveDate, NaiveDateTime};
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use std::fmt;

use crate::error::EvalError;

/// A computed value from the engine.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Exact decimal (money, percentages)
    Decimal(Decimal),
    /// IEEE-754 (scientific ops)
    Float(f64),
    /// Fixed-width integer (bit-ops, number system conversions)
    Integer(i128),
    /// Text
    Str(String),
    /// A value tagged with a physical unit (e.g. "10 km", "30 celsius")
    Unit {
        /// Magnitude in `unit`.
        amount: Decimal,
        /// Unit name as written, e.g. `km`, `miles`, `celsius`.
        unit: String,
    },
    /// A calendar date (e.g. `today`, `2026-07-04`)
    Date(NaiveDate),
    /// A wall-clock datetime (e.g. `now`)
    DateTime(NaiveDateTime),
    /// An error stored inline so a single bad line doesn't abort the session
    Err(String),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Decimal(d) => write!(f, "{d}"),
            Value::Float(v) => {
                // Trim trailing zeros after decimal point
                let s = format!("{v:.9}");
                let s = s.trim_end_matches('0').trim_end_matches('.');
                write!(f, "{s}")
            }
            Value::Integer(i) => write!(f, "{i}"),
            Value::Str(s) => write!(f, "{s}"),
            Value::Unit { amount, unit } => write!(f, "{} {unit}", amount.normalize()),
            Value::Date(d) => write!(f, "{}", d.format("%Y-%m-%d")),
            Value::DateTime(dt) => write!(f, "{}", dt.format("%Y-%m-%d %H:%M:%S")),
            Value::Err(e) => write!(f, "Error: {e}"),
        }
    }
}

impl Value {
    /// Promote to f64 for scientific operations.
    pub fn to_f64(&self) -> Option<f64> {
        match self {
            Value::Float(v) => Some(*v),
            Value::Decimal(d) => Some(rust_decimal::prelude::ToPrimitive::to_f64(d)?),
            Value::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Promote to Decimal for exact arithmetic.
    pub fn to_decimal(&self) -> Option<Decimal> {
        self.try_decimal()?.ok()
    }

    /// `None` for a non-numeric value, a range error for a number `Decimal` cannot hold.
    pub(crate) fn try_decimal(&self) -> Option<Result<Decimal, EvalError>> {
        match self {
            Value::Decimal(d) => Some(Ok(*d)),
            Value::Integer(i) => Some(int_to_decimal(*i)),
            Value::Float(v) => Some(
                Decimal::try_from(*v)
                    .map_err(|_| EvalError::TypeError(format!("{v} is out of Decimal range"))),
            ),
            _ => None,
        }
    }

    /// Return the integer representation if lossless.
    pub fn to_integer(&self) -> Option<i128> {
        match self {
            Value::Integer(i) => Some(*i),
            Value::Decimal(d) => {
                if d.fract().is_zero() {
                    rust_decimal::prelude::ToPrimitive::to_i128(d)
                } else {
                    None
                }
            }
            Value::Float(v) => {
                if v.fract() == 0.0 && v.abs() < i128::MAX as f64 {
                    Some(*v as i128)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

pub(crate) fn int_to_decimal(n: i128) -> Result<Decimal, EvalError> {
    Decimal::from_i128(n)
        .ok_or_else(|| EvalError::TypeError("integer is out of Decimal range".into()))
}
