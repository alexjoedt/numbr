use crate::error::EvalError;
use crate::value::Value;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;

/// Convert a `Value` to a new unit expressed by `target`.
///
/// Supported conversions:
/// - `Value::Unit { amount, unit }` → convert amount from `unit` to `target`
/// - `Value::Integer(n)` → label with target (treating n as days for duration targets)
/// - `Value::Decimal(d)` → label with target (treating d as days for duration targets)
/// - `Value::Float(f)` → label with target (treating f as days for duration targets)
pub fn convert_value(v: Value, target: &str) -> Result<Value, EvalError> {
    match v {
        Value::Unit { amount, ref unit } => {
            let a = amount
                .to_f64()
                .ok_or_else(|| EvalError::TypeError("unit amount must be numeric".into()))?;
            let result = convert_f64(a, unit, target)?;
            Ok(Value::Unit {
                amount: f64_to_decimal(result)?,
                unit: target.to_owned(),
            })
        }

        // A bare integer (e.g. from diff()) labelled with a duration unit
        Value::Integer(n) if is_duration_unit(target) => {
            let result = convert_f64(n as f64, "days", target)?;
            Ok(Value::Unit {
                amount: f64_to_decimal(result)?,
                unit: target.to_owned(),
            })
        }

        // A bare decimal labelled with a duration unit
        Value::Decimal(d) if is_duration_unit(target) => {
            let a = d.to_f64().ok_or_else(|| {
                EvalError::TypeError("decimal conversion requires numeric".into())
            })?;
            let result = convert_f64(a, "days", target)?;
            Ok(Value::Unit {
                amount: f64_to_decimal(result)?,
                unit: target.to_owned(),
            })
        }

        // A bare float literal labelled with a duration unit
        Value::Float(f) if is_duration_unit(target) => {
            let result = convert_f64(f, "days", target)?;
            Ok(Value::Unit {
                amount: f64_to_decimal(result)?,
                unit: target.to_owned(),
            })
        }

        other => Err(EvalError::TypeError(format!(
            "cannot convert '{other}' to unit '{target}'"
        ))),
    }
}

/// Core f64 conversion between two unit strings.
pub fn convert_f64(amount: f64, from: &str, to: &str) -> Result<f64, EvalError> {
    // Temperature — non-linear, handle separately
    if is_temperature_unit(from) && is_temperature_unit(to) {
        return convert_temperature(amount, from, to);
    }

    // Duration — linear via seconds
    if let (Some(from_s), Some(to_s)) = (duration_to_seconds(from), duration_to_seconds(to)) {
        return Ok(amount * from_s / to_s);
    }

    // Everything else — linear via SI/binary base
    let (from_base, from_factor) =
        linear_to_base(from).ok_or_else(|| EvalError::UnknownUnit(from.to_owned()))?;
    let (to_base, to_factor) =
        linear_to_base(to).ok_or_else(|| EvalError::UnknownUnit(to.to_owned()))?;

    if from_base != to_base {
        return Err(EvalError::TypeError(format!(
            "incompatible units: cannot convert '{from}' to '{to}'"
        )));
    }

    Ok(amount * from_factor / to_factor)
}

/// Returns `(base_unit_label, factor_to_base)` for linear (non-temperature, non-duration) units.
fn linear_to_base(unit: &str) -> Option<(&'static str, f64)> {
    Some(match unit {
        // ── Length  (base: m) ──────────────────────────────────────────────
        "m" | "meter" | "meters" => ("m", 1.0),
        "cm" | "centimeter" | "centimeters" => ("m", 0.01),
        "mm" | "millimeter" | "millimeters" => ("m", 0.001),
        "km" | "kilometer" | "kilometers" => ("m", 1_000.0),
        "mi" | "mile" | "miles" => ("m", 1_609.344),
        "ft" | "foot" | "feet" => ("m", 0.304_8),
        "in" | "inch" | "inches" => ("m", 0.025_4),
        "yd" | "yard" | "yards" => ("m", 0.914_4),

        // ── Mass  (base: kg) ───────────────────────────────────────────────
        "kg" | "kilogram" | "kilograms" => ("kg", 1.0),
        "g" | "gram" | "grams" => ("kg", 0.001),
        "mg" | "milligram" | "milligrams" => ("kg", 0.000_001),
        "lb" | "lbs" | "pound" | "pounds" => ("kg", 0.453_592_37),
        "oz" | "ounce" | "ounces" => ("kg", 0.028_349_523),

        // ── Pressure  (base: Pa) ───────────────────────────────────────────
        "pa" | "pascal" | "pascals" => ("pa", 1.0),
        "hpa" | "hectopascal" | "hectopascals" => ("pa", 100.0),
        "mbar" | "millibar" | "millibars" => ("pa", 100.0),
        "bar" | "bars" => ("pa", 100_000.0),
        "psi" => ("pa", 6_894.757_293),

        // ── Data — SI decimal bytes (base: B, factor of 1000) ─────────────
        "B" | "byte" | "bytes" => ("B", 1.0),
        "KB" | "kb" | "kilobyte" | "kilobytes" => ("B", 1_000.0),
        "MB" | "mb" | "megabyte" | "megabytes" => ("B", 1_000_000.0),
        "GB" | "gb" | "gigabyte" | "gigabytes" => ("B", 1_000_000_000.0),
        "TB" | "tb" | "terabyte" | "terabytes" => ("B", 1_000_000_000_000.0),
        "PB" | "pb" | "petabyte" | "petabytes" => ("B", 1_000_000_000_000_000.0),
        "EB" | "eb" | "exabyte" | "exabytes" => ("B", 1_000_000_000_000_000_000.0),

        // ── Data — IEC binary bytes (base: B, factor of 1024) ────────────
        "KiB" | "kibibyte" | "kibibytes" => ("B", 1_024.0),
        "MiB" | "mebibyte" | "mebibytes" => ("B", 1_048_576.0),
        "GiB" | "gibibyte" | "gibibytes" => ("B", 1_073_741_824.0),
        "TiB" | "tebibyte" | "tebibytes" => ("B", 1_099_511_627_776.0),
        "PiB" | "pebibyte" | "pebibytes" => ("B", 1_125_899_906_842_624.0),
        "EiB" | "exbibyte" | "exbibytes" => ("B", 1_152_921_504_606_846_976.0),

        // ── Data — bits, SI decimal (base: bit, factor of 1000) ───────────
        "bit" | "bits" => ("bit", 1.0),
        "kbit" | "kilobit" | "kilobits" => ("bit", 1_000.0),
        "mbit" | "megabit" | "megabits" => ("bit", 1_000_000.0),
        "gbit" | "gigabit" | "gigabits" => ("bit", 1_000_000_000.0),
        "tbit" | "terabit" | "terabits" => ("bit", 1_000_000_000_000.0),
        "pbit" | "petabit" | "petabits" => ("bit", 1_000_000_000_000_000.0),
        "ebit" | "exabit" | "exabits" => ("bit", 1_000_000_000_000_000_000.0),

        _ => return None,
    })
}

fn is_temperature_unit(unit: &str) -> bool {
    matches!(
        unit,
        "celsius" | "fahrenheit" | "kelvin" | "degC" | "degF" | "degK" | "C" | "F" | "K"
    )
}

/// Whether `unit` is a duration (time) unit.
pub fn is_duration_unit(unit: &str) -> bool {
    duration_to_seconds(unit).is_some()
}

/// Whether `unit` is any recognized unit (length, mass, temp, pressure, data, or duration).
pub fn is_known_unit(unit: &str) -> bool {
    linear_to_base(unit).is_some() || is_temperature_unit(unit) || is_duration_unit(unit)
}

fn convert_temperature(amount: f64, from: &str, to: &str) -> Result<f64, EvalError> {
    let kelvin = match from {
        "celsius" | "degC" | "C" => amount + 273.15,
        "fahrenheit" | "degF" | "F" => (amount - 32.0) * 5.0 / 9.0 + 273.15,
        "kelvin" | "degK" | "K" => amount,
        _ => return Err(EvalError::UnknownUnit(from.to_owned())),
    };
    Ok(match to {
        "celsius" | "degC" | "C" => kelvin - 273.15,
        "fahrenheit" | "degF" | "F" => (kelvin - 273.15) * 9.0 / 5.0 + 32.0,
        "kelvin" | "degK" | "K" => kelvin,
        _ => return Err(EvalError::UnknownUnit(to.to_owned())),
    })
}

/// Returns the conversion factor from `unit` to seconds.
/// Returns `None` when `unit` is not a duration unit.
pub fn duration_to_seconds(unit: &str) -> Option<f64> {
    Some(match unit {
        "ms" | "millisecond" | "milliseconds" => 0.001,
        "s" | "sec" | "second" | "seconds" => 1.0,
        "min" | "minute" | "minutes" => 60.0,
        "h" | "hour" | "hours" => 3_600.0,
        "day" | "days" => 86_400.0,
        "week" | "weeks" => 604_800.0,
        "month" | "months" => 2_629_800.0, // average month (30.4375 d)
        "year" | "years" => 31_557_600.0,  // Julian year
        _ => return None,
    })
}

/// Returns whole days per calendar unit for date arithmetic.
/// Returns `None` when `unit` is not a calendar unit suitable for date offsets.
pub fn time_unit_to_days(unit: &str) -> Option<i64> {
    Some(match unit {
        "day" | "days" => 1,
        "week" | "weeks" => 7,
        "month" | "months" => 30,
        "year" | "years" => 365,
        _ => return None,
    })
}

/// Returns whole seconds per time unit for datetime arithmetic.
pub fn time_unit_to_seconds(unit: &str) -> Option<i64> {
    Some(match unit {
        "s" | "sec" | "second" | "seconds" => 1,
        "min" | "minute" | "minutes" => 60,
        "h" | "hour" | "hours" => 3_600,
        "day" | "days" => 86_400,
        "week" | "weeks" => 604_800,
        "month" | "months" => 2_629_800,
        "year" | "years" => 31_557_600,
        _ => return None,
    })
}

/// Returns whole milliseconds per time unit for datetime arithmetic.
pub fn time_unit_to_milliseconds(unit: &str) -> Option<i64> {
    Some(match unit {
        "ms" | "millisecond" | "milliseconds" => 1,
        "s" | "sec" | "second" | "seconds" => 1_000,
        "min" | "minute" | "minutes" => 60_000,
        "h" | "hour" | "hours" => 3_600_000,
        "day" | "days" => 86_400_000,
        "week" | "weeks" => 604_800_000,
        "month" | "months" => 2_629_800_000,
        "year" | "years" => 31_557_600_000,
        _ => return None,
    })
}

/// Convert an f64 result to a `Decimal` rounded to 10 decimal places.
pub fn f64_to_decimal(v: f64) -> Result<Decimal, EvalError> {
    Decimal::from_f64(v)
        .map(|d| d.round_dp(10))
        .ok_or_else(|| EvalError::TypeError("unit conversion produced a non-finite value".into()))
}
