use rust_decimal::Decimal;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::str::FromStr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RateError {
    #[error("Currency '{0}' not found in rates")]
    NotFound(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(String),
}

pub trait RateProvider: Send + Sync {
    fn rates(&self, base: &str) -> Result<BTreeMap<String, Decimal>, RateError>;
}

/// Reads rates from a local JSON cache (`~/.cache/numbr/rates.json`).
pub struct OfflineProvider {
    cache_path: std::path::PathBuf,
}

impl OfflineProvider {
    pub fn new(cache_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            cache_path: cache_path.into(),
        }
    }
}

impl RateProvider for OfflineProvider {
    fn rates(&self, base: &str) -> Result<BTreeMap<String, Decimal>, RateError> {
        let content = std::fs::read_to_string(&self.cache_path)?;
        parse_cache(&content, base)
    }
}

fn parse_cache(content: &str, base: &str) -> Result<BTreeMap<String, Decimal>, RateError> {
    let json: Value =
        serde_json::from_str(content).map_err(|err| RateError::Parse(err.to_string()))?;

    // Supports either a flat `{ "USD": "1.0", ... }` cache or a nested
    // `{ "rates": { "USD": { ... }, "EUR": { ... } } }` cache. Flat
    // caches are assumed to already be expressed in the caller's base.
    let rates = json.get("rates").unwrap_or(&json);
    let rates = rates
        .get(base)
        .and_then(Value::as_object)
        .or_else(|| rates.as_object())
        .ok_or_else(|| RateError::Parse("rates cache must contain a JSON object".into()))?;

    let rates = parse_rate_map(rates)?;
    if rates.is_empty() {
        Err(RateError::NotFound(base.to_owned()))
    } else {
        Ok(rates)
    }
}

fn parse_rate_map(rates: &Map<String, Value>) -> Result<BTreeMap<String, Decimal>, RateError> {
    rates
        .iter()
        .map(|(currency, value)| {
            parse_decimal_value(currency, value).map(|rate| (currency.to_owned(), rate))
        })
        .collect()
}

fn parse_decimal_value(currency: &str, value: &Value) -> Result<Decimal, RateError> {
    let raw = match value {
        Value::String(value) => value.as_str(),
        Value::Number(value) => {
            return Decimal::from_str(&value.to_string()).map_err(|err| {
                RateError::Parse(format!("invalid decimal rate for {currency}: {err}"))
            })
        }
        _ => {
            return Err(RateError::Parse(format!(
                "rate for {currency} must be a string or number"
            )))
        }
    };

    Decimal::from_str(raw)
        .map_err(|err| RateError::Parse(format!("invalid decimal rate for {currency}: {err}")))
}
