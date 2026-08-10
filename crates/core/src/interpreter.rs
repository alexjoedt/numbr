use chrono::Duration;
use rust_decimal::Decimal;

use crate::error::{EvalError, FuncError};
use crate::functions::FunctionProvider;
use crate::parser::{BinOp, BitCast, Expr, UnOp};
use crate::scope::Scope;
use crate::units;
use crate::value::Value;

pub struct Interpreter {
    pub scope: Scope,
    providers: Vec<Box<dyn FunctionProvider>>,
}

impl Interpreter {
    pub fn new(providers: Vec<Box<dyn FunctionProvider>>) -> Self {
        Self {
            scope: Scope::new(),
            providers,
        }
    }

    /// Create an interpreter pre-seeded with an existing scope snapshot.
    pub fn with_scope(scope: Scope, providers: Vec<Box<dyn FunctionProvider>>) -> Self {
        Self { scope, providers }
    }

    /// Evaluate an already-parsed expression.
    pub fn eval(&mut self, expr: &Expr) -> Result<Value, EvalError> {
        match expr {
            Expr::Integer(n) => Ok(Value::Integer(*n)),
            Expr::Float(f) => Ok(Value::Float(*f)),
            Expr::Str(s) => Ok(Value::Str(s.clone())),
            Expr::Date(d) => Ok(Value::Date(*d)),

            Expr::UnitValue { amount, unit } => {
                let v = self.eval(amount)?;
                let a = v.to_decimal().ok_or_else(|| {
                    EvalError::TypeError(format!("unit value must be numeric, got '{v}'"))
                })?;
                Ok(Value::Unit {
                    amount: a,
                    unit: unit.clone(),
                })
            }

            Expr::Ident(name) => {
                // Well-known constants and date/time keywords
                match name.as_str() {
                    "pi" | "PI" => return Ok(Value::Float(std::f64::consts::PI)),
                    "e" | "E" => return Ok(Value::Float(std::f64::consts::E)),
                    "phi" => return Ok(Value::Float(1.618_033_988_749_895)),
                    "today" => {
                        return Ok(Value::Date(chrono::Local::now().date_naive()));
                    }
                    "now" => {
                        return Ok(Value::DateTime(chrono::Local::now().naive_local()));
                    }
                    // Modbus register-order constants — pass through as strings so
                    // modbus::float32(r1, r2, CDAB) works without quoting.
                    "ABCD" | "CDAB" | "BADC" | "DCBA" => {
                        return Ok(Value::Str(name.clone()));
                    }
                    _ => {}
                }
                self.scope
                    .resolve(name)
                    .ok_or_else(|| EvalError::UnknownVariable(name.clone()))
            }

            Expr::Assign { name, value } => {
                let v = self.eval(value)?;
                self.scope.set_var(name, v.clone());
                Ok(v)
            }

            Expr::Sequence(exprs) => {
                let mut last = Value::Integer(0);
                for e in exprs {
                    last = self.eval(e)?;
                }
                Ok(last)
            }

            Expr::UnaryOp { op, operand } => {
                let v = self.eval(operand)?;
                self.eval_unary(op, v)
            }

            Expr::BinaryOp { op, left, right } => {
                // Special case: `X + N%` / `X - N%` means "increase / decrease X by N%".
                // We intercept before evaluating `right` so we see the raw Percent node.
                if matches!(op, BinOp::Add | BinOp::Sub) {
                    if let Expr::Percent(inner) = right.as_ref() {
                        let l = self.eval(left)?;
                        let pct_num = self.eval(inner)?; // raw number before /100
                        let pct_frac = match &pct_num {
                            Value::Integer(n) => {
                                Value::Decimal(Decimal::from(*n) / Decimal::ONE_HUNDRED)
                            }
                            Value::Decimal(d) => Value::Decimal(d / Decimal::ONE_HUNDRED),
                            Value::Float(f) => Value::Float(f / 100.0),
                            other => {
                                return Err(EvalError::TypeError(format!(
                                    "% applied to non-numeric value: {other}"
                                )))
                            }
                        };
                        let adjustment = mul_values(l.clone(), pct_frac)?;
                        return match op {
                            BinOp::Add => add_values(l, adjustment),
                            BinOp::Sub => sub_values(l, adjustment),
                            _ => unreachable!(),
                        };
                    }
                }
                let l = self.eval(left)?;
                let r = self.eval(right)?;
                self.eval_binary(op, l, r)
            }

            Expr::Call { name, args } => {
                let mut evaluated = Vec::with_capacity(args.len());
                for a in args {
                    evaluated.push(self.eval(a)?);
                }
                self.call_function(name, &evaluated)
                    .map_err(EvalError::FuncError)
            }

            Expr::BitCast { value, cast } => {
                let v = self.eval(value)?;
                self.apply_bit_cast(v, cast)
            }

            Expr::Convert { value, target } => {
                let v = self.eval(value)?;
                self.eval_convert(v, target)
            }

            Expr::Percent(inner) => {
                let v = self.eval(inner)?;
                // Standalone percent → divide by 100 (Decimal for precision)
                match v {
                    Value::Integer(n) => {
                        Ok(Value::Decimal(Decimal::from(n) / Decimal::ONE_HUNDRED))
                    }
                    Value::Decimal(d) => Ok(Value::Decimal(d / Decimal::ONE_HUNDRED)),
                    Value::Float(f) => Ok(Value::Float(f / 100.0)),
                    other => Err(EvalError::TypeError(format!(
                        "% applied to non-numeric value: {other}"
                    ))),
                }
            }

            Expr::PercentOf { percent, value } => {
                // `X% of Y` → Y * X / 100
                let pct = self.eval(percent)?;
                let base = self.eval(value)?;
                // Percent evaluates to X/100 already, so multiply by that fraction.
                mul_values(base, pct)
            }
        }
    }

    fn eval_unary(&self, op: &UnOp, v: Value) -> Result<Value, EvalError> {
        match op {
            UnOp::Neg => match v {
                Value::Integer(n) => Ok(Value::Integer(-n)),
                Value::Decimal(d) => Ok(Value::Decimal(-d)),
                Value::Float(f) => Ok(Value::Float(-f)),
                Value::Unit { amount, unit } => Ok(Value::Unit {
                    amount: -amount,
                    unit,
                }),
                other => Err(EvalError::TypeError(format!("cannot negate {other}"))),
            },
            UnOp::BitNot => match v {
                Value::Integer(n) => Ok(Value::Integer(!n)),
                other => Err(EvalError::TypeError(format!(
                    "bitwise NOT requires integer, got {other}"
                ))),
            },
        }
    }

    fn eval_binary(&self, op: &BinOp, l: Value, r: Value) -> Result<Value, EvalError> {
        match op {
            BinOp::Add => add_values(l, r),
            BinOp::Sub => sub_values(l, r),
            BinOp::Mul => mul_values(l, r),
            BinOp::Div => div_values(l, r),
            BinOp::Rem => rem_values(l, r),
            BinOp::Pow => pow_values(l, r),
            BinOp::BitAnd => bitwise(l, r, |a, b| a & b, "&"),
            BinOp::BitOr => bitwise(l, r, |a, b| a | b, "|"),
            BinOp::BitXor => bitwise(l, r, |a, b| a ^ b, "^"),
            BinOp::Shl => shift(l, r, "<<", i128::checked_shl),
            BinOp::Shr => shift(l, r, ">>", i128::checked_shr),
        }
    }

    fn apply_bit_cast(&self, v: Value, cast: &BitCast) -> Result<Value, EvalError> {
        let n = v
            .to_integer()
            .ok_or_else(|| EvalError::TypeError("bit cast requires integer value".into()))?;

        let result = match (cast.signed, cast.bits) {
            (false, 8) => (n as u8) as i128,
            (false, 16) => (n as u16) as i128,
            (false, 32) => (n as u32) as i128,
            (false, 64) => (n as u64) as i128,
            (true, 8) => (n as i8) as i128,
            (true, 16) => (n as i16) as i128,
            (true, 32) => (n as i32) as i128,
            (true, 64) => (n as i64) as i128,
            _ => {
                return Err(EvalError::TypeError(format!(
                    "unsupported bit width {}",
                    cast.bits
                )))
            }
        };

        Ok(Value::Integer(result))
    }

    fn eval_convert(&self, v: Value, target: &str) -> Result<Value, EvalError> {
        match target {
            "hex" | "hexadecimal" => {
                let n = v.to_integer().ok_or_else(|| {
                    EvalError::TypeError("hex conversion requires integer".into())
                })?;
                Ok(Value::Str(format!("{:#X}", n)))
            }
            "binary" | "bin" => {
                let n = v.to_integer().ok_or_else(|| {
                    EvalError::TypeError("binary conversion requires integer".into())
                })?;
                Ok(Value::Str(format!("{:#b}", n)))
            }
            "octal" | "oct" => {
                let n = v.to_integer().ok_or_else(|| {
                    EvalError::TypeError("octal conversion requires integer".into())
                })?;
                Ok(Value::Str(format!("{:#o}", n)))
            }
            "decimal" | "dec" => {
                let n = v.to_integer().ok_or_else(|| {
                    EvalError::TypeError("decimal conversion requires integer".into())
                })?;
                Ok(Value::Integer(n))
            }
            // Delegate to units module for unit/duration conversions
            other => units::convert_value(v, other),
        }
    }

    fn call_function(&self, name: &str, args: &[Value]) -> Result<Value, FuncError> {
        for provider in &self.providers {
            if provider.provides(name) {
                return provider.call(name, args);
            }
        }
        Err(FuncError::NotFound(name.to_owned()))
    }
}

// ── Arithmetic helpers ────────────────────────────────────────────────────────

fn promote(l: Value, r: Value) -> (Value, Value) {
    // Integer + Decimal → Decimal
    // Anything + Float → Float
    match (l, r) {
        (Value::Float(lf), r) => match r.to_f64() {
            Some(rf) => (Value::Float(lf), Value::Float(rf)),
            None => (Value::Float(lf), r),
        },
        (l, Value::Float(rf)) => match l.to_f64() {
            Some(lf) => (Value::Float(lf), Value::Float(rf)),
            None => (l, Value::Float(rf)),
        },
        (l @ Value::Decimal(_), Value::Integer(i)) => {
            let rd = Decimal::from(i);
            (l, Value::Decimal(rd))
        }
        (Value::Integer(i), r @ Value::Decimal(_)) => {
            let ld = Decimal::from(i);
            (Value::Decimal(ld), r)
        }
        (l, r) => (l, r),
    }
}

fn add_values(l: Value, r: Value) -> Result<Value, EvalError> {
    // Date + duration unit  (e.g. `today + 2 weeks`)
    if let (
        Value::Date(d),
        Value::Unit {
            ref amount,
            ref unit,
        },
    ) = (&l, &r)
    {
        return add_date_duration(*d, amount, unit);
    }
    // DateTime + duration unit  (e.g. `now + 3 hours`)
    if let (
        Value::DateTime(dt),
        Value::Unit {
            ref amount,
            ref unit,
        },
    ) = (&l, &r)
    {
        return add_datetime_duration(*dt, amount, unit);
    }

    let (l, r) = promote(l, r);
    match (l, r) {
        (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a.wrapping_add(b))),
        (Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal(a + b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
        (Value::Str(a), Value::Str(b)) => Ok(Value::Str(a + &b)),
        // Unit + Unit (same category) — convert RHS to LHS unit and add
        (
            Value::Unit {
                amount: la,
                unit: lu,
            },
            Value::Unit {
                amount: ra,
                unit: ru,
            },
        ) => {
            use rust_decimal::prelude::ToPrimitive;
            let ra = ra
                .to_f64()
                .ok_or_else(|| EvalError::TypeError("unit amount must be numeric".into()))?;
            let rf = units::convert_f64(ra, &ru, &lu)?;
            Ok(Value::Unit {
                amount: la + units::f64_to_decimal(rf)?,
                unit: lu,
            })
        }
        (l, r) => Err(EvalError::TypeError(format!("cannot add {l} and {r}"))),
    }
}

fn sub_values(l: Value, r: Value) -> Result<Value, EvalError> {
    // Date - duration unit  (e.g. `today - 3 days`)
    if let (
        Value::Date(d),
        Value::Unit {
            ref amount,
            ref unit,
        },
    ) = (&l, &r)
    {
        return add_date_duration(*d, &(-*amount), unit);
    }
    // DateTime - duration unit  (e.g. `now - 3 hours`)
    if let (
        Value::DateTime(dt),
        Value::Unit {
            ref amount,
            ref unit,
        },
    ) = (&l, &r)
    {
        return add_datetime_duration(*dt, &(-*amount), unit);
    }
    // Date - Date  → integer days
    if let (Value::Date(d1), Value::Date(d2)) = (&l, &r) {
        return Ok(Value::Integer((*d1 - *d2).num_days().into()));
    }
    // DateTime - DateTime → integer seconds
    if let (Value::DateTime(dt1), Value::DateTime(dt2)) = (&l, &r) {
        return Ok(Value::Integer((*dt1 - *dt2).num_seconds().into()));
    }

    let (l, r) = promote(l, r);
    match (l, r) {
        (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a.wrapping_sub(b))),
        (Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal(a - b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
        (l, r) => Err(EvalError::TypeError(format!(
            "cannot subtract {r} from {l}"
        ))),
    }
}

/// Add a duration (expressed as a `Value::Unit`) to a `NaiveDate`.
fn add_date_duration(
    d: chrono::NaiveDate,
    amount: &Decimal,
    unit: &str,
) -> Result<Value, EvalError> {
    if let Some(days_per_unit) = units::time_unit_to_days(unit) {
        let amount = decimal_to_f64(amount, "duration amount")?;
        let total_days = rounded_f64_to_i64(amount * days_per_unit as f64, "duration days")?;
        let new_date = Duration::try_days(total_days)
            .and_then(|delta| d.checked_add_signed(delta))
            .ok_or_else(|| EvalError::TypeError("date out of range".into()))?;
        Ok(Value::Date(new_date))
    } else if units::is_duration_unit(unit) {
        // Sub-day precision: round to nearest day
        let secs_per_unit = units::duration_to_seconds(unit)
            .ok_or_else(|| EvalError::TypeError(format!("cannot add unit '{unit}' to a date")))?;
        let amount = decimal_to_f64(amount, "duration amount")?;
        let total_days = rounded_f64_to_i64(amount * secs_per_unit / 86_400.0, "duration days")?;
        let new_date = Duration::try_days(total_days)
            .and_then(|delta| d.checked_add_signed(delta))
            .ok_or_else(|| EvalError::TypeError("date out of range".into()))?;
        Ok(Value::Date(new_date))
    } else {
        Err(EvalError::TypeError(format!(
            "cannot add unit '{unit}' to a date"
        )))
    }
}

/// Add a duration (expressed as a `Value::Unit`) to a `NaiveDateTime`.
fn add_datetime_duration(
    dt: chrono::NaiveDateTime,
    amount: &Decimal,
    unit: &str,
) -> Result<Value, EvalError> {
    if let Some(ms_per_unit) = units::time_unit_to_milliseconds(unit) {
        let amount = decimal_to_f64(amount, "duration amount")?;
        let total_ms = rounded_f64_to_i64(amount * ms_per_unit as f64, "duration milliseconds")?;
        let new_dt = Duration::try_milliseconds(total_ms)
            .and_then(|delta| dt.checked_add_signed(delta))
            .ok_or_else(|| EvalError::TypeError("datetime out of range".into()))?;
        Ok(Value::DateTime(new_dt))
    } else {
        Err(EvalError::TypeError(format!(
            "cannot add unit '{unit}' to a datetime"
        )))
    }
}

fn mul_values(l: Value, r: Value) -> Result<Value, EvalError> {
    let (l, r) = promote(l, r);
    match (l, r) {
        (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a.wrapping_mul(b))),
        (Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal(a * b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
        (Value::Unit { amount, unit }, scalar) | (scalar, Value::Unit { amount, unit }) => {
            let scalar = scalar.to_decimal().ok_or_else(|| {
                EvalError::TypeError("unit multiplication requires a numeric scalar".into())
            })?;
            Ok(Value::Unit {
                amount: amount * scalar,
                unit,
            })
        }
        (l, r) => Err(EvalError::TypeError(format!("cannot multiply {l} and {r}"))),
    }
}

fn div_values(l: Value, r: Value) -> Result<Value, EvalError> {
    // Check for zero before promoting
    match &r {
        Value::Integer(0) => return Err(EvalError::DivisionByZero),
        Value::Decimal(d) if d.is_zero() => return Err(EvalError::DivisionByZero),
        Value::Float(f) if *f == 0.0 => return Err(EvalError::DivisionByZero),
        _ => {}
    }
    let (l, r) = promote(l, r);
    match (l, r) {
        (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a / b)),
        (Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal(a / b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
        (Value::Unit { amount, unit }, scalar) => {
            let scalar = scalar.to_decimal().ok_or_else(|| {
                EvalError::TypeError("unit division requires a numeric scalar".into())
            })?;
            Ok(Value::Unit {
                amount: amount / scalar,
                unit,
            })
        }
        (l, r) => Err(EvalError::TypeError(format!("cannot divide {l} by {r}"))),
    }
}

fn rem_values(l: Value, r: Value) -> Result<Value, EvalError> {
    match &r {
        Value::Integer(0) => return Err(EvalError::DivisionByZero),
        Value::Decimal(d) if d.is_zero() => return Err(EvalError::DivisionByZero),
        Value::Float(f) if *f == 0.0 => return Err(EvalError::DivisionByZero),
        _ => {}
    }
    let (l, r) = promote(l, r);
    match (l, r) {
        (Value::Integer(a), Value::Integer(b)) => Ok(Value::Integer(a % b)),
        (Value::Decimal(a), Value::Decimal(b)) => Ok(Value::Decimal(a % b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a % b)),
        (l, r) => Err(EvalError::TypeError(format!("cannot mod {l} by {r}"))),
    }
}

fn pow_values(l: Value, r: Value) -> Result<Value, EvalError> {
    match (&l, &r) {
        (Value::Integer(base), Value::Integer(exp)) if *exp >= 0 => {
            let exp = u32::try_from(*exp)
                .map_err(|_| EvalError::TypeError("integer exponent is out of range".into()))?;
            base.checked_pow(exp)
                .map(Value::Integer)
                .ok_or_else(|| EvalError::TypeError("integer power overflowed i128".into()))
        }
        _ => {
            let base = l
                .to_f64()
                .ok_or_else(|| EvalError::TypeError("pow requires numeric".into()))?;
            let exp = r
                .to_f64()
                .ok_or_else(|| EvalError::TypeError("pow requires numeric".into()))?;
            Ok(Value::Float(base.powf(exp)))
        }
    }
}

fn bitwise<F: Fn(i128, i128) -> i128>(
    l: Value,
    r: Value,
    op: F,
    sym: &str,
) -> Result<Value, EvalError> {
    let a = l
        .to_integer()
        .ok_or_else(|| EvalError::TypeError(format!("bitwise {sym} requires integer operands")))?;
    let b = r
        .to_integer()
        .ok_or_else(|| EvalError::TypeError(format!("bitwise {sym} requires integer operands")))?;
    Ok(Value::Integer(op(a, b)))
}

fn shift(
    l: Value,
    r: Value,
    sym: &str,
    op: fn(i128, u32) -> Option<i128>,
) -> Result<Value, EvalError> {
    let a = l
        .to_integer()
        .ok_or_else(|| EvalError::TypeError(format!("shift {sym} requires integer operands")))?;
    let b = r
        .to_integer()
        .ok_or_else(|| EvalError::TypeError(format!("shift {sym} requires integer operands")))?;
    let b = u32::try_from(b)
        .map_err(|_| EvalError::TypeError(format!("shift count for {sym} must be non-negative")))?;

    op(a, b)
        .map(Value::Integer)
        .ok_or_else(|| EvalError::TypeError(format!("shift count out of range for {sym}")))
}

fn decimal_to_f64(value: &Decimal, label: &str) -> Result<f64, EvalError> {
    use rust_decimal::prelude::ToPrimitive;

    value
        .to_f64()
        .filter(|value| value.is_finite())
        .ok_or_else(|| EvalError::TypeError(format!("{label} is out of range")))
}

fn rounded_f64_to_i64(value: f64, label: &str) -> Result<i64, EvalError> {
    if !value.is_finite() || value < i64::MIN as f64 || value > i64::MAX as f64 {
        return Err(EvalError::TypeError(format!("{label} is out of range")));
    }

    Ok(value.round() as i64)
}
