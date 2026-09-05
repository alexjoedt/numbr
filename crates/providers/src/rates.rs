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

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(raw: &str) -> Decimal {
        Decimal::from_str(raw).unwrap()
    }

    #[test]
    fn parses_a_flat_cache() {
        // A flat cache has no entry named after the base, so the `get(base)`
        // lookup misses and the whole map is used as-is. That is the
        // documented intent: flat caches are already in the caller's base.
        let rates = parse_cache(r#"{"USD":"1.0","EUR":"0.92"}"#, "USD").unwrap();
        assert_eq!(rates.len(), 2);
        assert_eq!(rates["USD"], dec("1.0"));
        assert_eq!(rates["EUR"], dec("0.92"));
    }

    #[test]
    fn parses_a_flat_cache_under_an_unknown_base() {
        let rates = parse_cache(r#"{"USD":"1.0","EUR":"0.92"}"#, "GBP").unwrap();
        assert_eq!(rates.len(), 2);
    }

    #[test]
    fn parses_a_nested_cache() {
        let rates = parse_cache(r#"{"rates":{"USD":{"EUR":"0.92"}}}"#, "USD").unwrap();
        assert_eq!(rates.len(), 1);
        assert_eq!(rates["EUR"], dec("0.92"));
    }

    #[test]
    fn parses_numeric_rates() {
        let rates = parse_cache(r#"{"USD":1.0,"EUR":0.92}"#, "USD").unwrap();
        assert_eq!(rates["USD"], dec("1.0"));
        assert_eq!(rates["EUR"], dec("0.92"));
    }

    #[test]
    fn reads_a_cache_from_disk() {
        let path = std::env::temp_dir().join("numbr-offline-provider-test.json");
        std::fs::write(&path, r#"{"USD":"1.0","EUR":"0.92"}"#).unwrap();
        let rates = OfflineProvider::new(&path).rates("USD").unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(rates["EUR"], dec("0.92"));
    }

    #[test]
    fn missing_file_is_an_io_error() {
        let provider = OfflineProvider::new("/nonexistent/numbr/rates.json");
        assert!(matches!(provider.rates("USD"), Err(RateError::Io(_))));
    }

    #[test]
    fn malformed_json_is_a_parse_error() {
        assert!(matches!(
            parse_cache("{not json", "USD"),
            Err(RateError::Parse(_))
        ));
    }

    #[test]
    fn non_object_json_is_a_parse_error() {
        for content in [r#"[1,2,3]"#, r#""hello""#] {
            let err = parse_cache(content, "USD").unwrap_err();
            assert!(
                matches!(&err, RateError::Parse(msg) if msg.contains("must contain a JSON object")),
                "unexpected error for {content}: {err}"
            );
        }
    }

    #[test]
    fn invalid_decimal_names_the_currency() {
        let err = parse_cache(r#"{"USD":"not-a-number"}"#, "USD").unwrap_err();
        assert!(
            matches!(&err, RateError::Parse(msg) if msg.contains("USD")),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn out_of_range_numeric_rate_names_the_currency() {
        // Syntactically valid JSON numbers can still exceed the range of a
        // Decimal, which is the error arm of the `Value::Number` branch.
        let err = parse_cache(r#"{"USD":1e40}"#, "USD").unwrap_err();
        assert!(
            matches!(&err, RateError::Parse(msg) if msg.contains("USD")),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn wrong_typed_rate_is_a_parse_error() {
        for content in [r#"{"USD":true}"#, r#"{"USD":null}"#] {
            let err = parse_cache(content, "USD").unwrap_err();
            assert!(
                matches!(&err, RateError::Parse(msg) if msg.contains("must be a string or number")),
                "unexpected error for {content}: {err}"
            );
        }
    }

    #[test]
    fn empty_cache_reports_the_base_as_not_found() {
        assert!(matches!(
            parse_cache("{}", "USD"),
            Err(RateError::NotFound(base)) if base == "USD"
        ));
    }
}
