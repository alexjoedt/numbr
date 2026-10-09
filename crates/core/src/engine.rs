use crate::builtin::BuiltinFunctions;
use crate::error::EvalError;
use crate::functions::FunctionProvider;
use crate::interpreter::Interpreter;
use crate::modbus::ModbusFunctions;
use crate::parser::{self, DecimalSeparator};
use crate::scope::Scope;
use crate::value::Value;
use rust_decimal::Decimal;

/// The public API of the `numbr-core` engine.
pub struct Engine {
    interpreter: Interpreter,
    decimal_separator: DecimalSeparator,
}

impl Engine {
    /// Create a new engine with the built-in and `modbus::` functions.
    ///
    /// # Examples
    ///
    /// ```
    /// use numbr_core::{Engine, Value};
    ///
    /// let mut engine = Engine::new();
    /// assert_eq!(engine.evaluate("sqrt(16)").unwrap(), Value::Float(4.0));
    /// assert!(engine.scope().lines().is_empty());
    /// ```
    pub fn new() -> Self {
        Self::with_providers(vec![Box::new(BuiltinFunctions), Box::new(ModbusFunctions)])
    }

    /// Create a new engine with custom providers (for dependency injection).
    pub fn with_providers(providers: Vec<Box<dyn FunctionProvider>>) -> Self {
        Self {
            interpreter: Interpreter::new(providers),
            decimal_separator: DecimalSeparator::default(),
        }
    }

    /// Create a new engine seeded with an existing scope snapshot.
    /// Used for incremental evaluation — only lines from `first_dirty` onwards
    /// need re-evaluation; the scope state before that point is restored here.
    pub fn with_scope(scope: Scope) -> Self {
        Self {
            interpreter: Interpreter::with_scope(
                scope,
                vec![Box::new(BuiltinFunctions), Box::new(ModbusFunctions)],
            ),
            decimal_separator: DecimalSeparator::default(),
        }
    }

    /// Set which character is read as the decimal separator in number literals.
    pub fn with_decimal_separator(mut self, separator: DecimalSeparator) -> Self {
        self.decimal_separator = separator;
        self
    }

    /// Snapshot of the current scope (variables + line results recorded so far).
    pub fn scope(&self) -> &Scope {
        &self.interpreter.scope
    }

    /// Evaluate a single line. Returns the result value or an error.
    ///
    /// Comments (`# ...`) are stripped first. An empty line, or a prefix of `result`,
    /// yields an empty `Value::Str`. `result: <aggregate>` aggregates the numeric results
    /// recorded by [`Engine::evaluate_line`] above it. Assignments update the scope, but
    /// unlike [`Engine::evaluate_line`] the result is not recorded for `lineN`.
    ///
    /// # Errors
    ///
    /// [`EvalError::ParseError`] for invalid syntax, [`EvalError::Incomplete`] for an
    /// unknown `result:` aggregate, and the evaluation errors
    /// [`EvalError::DivisionByZero`], [`EvalError::UnknownVariable`],
    /// [`EvalError::TypeError`], [`EvalError::UnknownUnit`], [`EvalError::FuncError`].
    ///
    /// # Panics
    ///
    /// Overflow outside the `**` path is not turned into an error:
    ///
    /// - An integer beyond the `Decimal` range (about 7.9e28) panics when it is promoted
    ///   to `Decimal`: as a unit amount, a percentage or next to a `Decimal` operand,
    ///   e.g. `100000000000000000000000000000 km` or `100000000000000000000000000000%`.
    /// - `Decimal` arithmetic is unchecked, so a result beyond its range panics, e.g.
    ///   `79228162514264337593543950335 km * 10` or `10 km / 0.0000000000000000000000000001`.
    /// - Prefix `-`, `/ -1` and `mod -1` on `i128::MIN` panic (prefix `-` wraps in
    ///   release builds).
    ///
    /// Integer `+`, `-` and `*` never panic: they wrap silently on `i128` overflow, so
    /// `170141183460469231731687303715884105727 + 1` gives `i128::MIN`. Only `**`
    /// checks and returns [`EvalError::TypeError`] on overflow.
    ///
    /// # Examples
    ///
    /// ```
    /// use numbr_core::{Engine, EvalError, Value};
    ///
    /// let mut engine = Engine::new();
    /// assert_eq!(engine.evaluate("x = 6 * 7").unwrap(), Value::Integer(42));
    /// assert_eq!(engine.evaluate("x / 2  # half").unwrap(), Value::Integer(21));
    /// assert_eq!(engine.evaluate("1 / 0"), Err(EvalError::DivisionByZero));
    /// assert!(matches!(engine.evaluate("2 +"), Err(EvalError::ParseError { .. })));
    /// ```
    pub fn evaluate(&mut self, input: &str) -> Result<Value, EvalError> {
        let trimmed = strip_comment(input).trim();
        if trimmed.is_empty() {
            return Ok(Value::Str(String::new()));
        }
        if "result".starts_with(trimmed) {
            return Ok(Value::Str(String::new()));
        }
        if let Some(command) = trimmed.strip_prefix("result:") {
            return self.evaluate_result_command(command.trim());
        }
        let ast = parser::parse_with(trimmed, self.decimal_separator)?;
        self.interpreter.eval(&ast)
    }

    fn evaluate_result_command(&self, command: &str) -> Result<Value, EvalError> {
        let command = command.to_ascii_lowercase();
        let numbers = contiguous_result_numbers(self.interpreter.scope.lines());

        match command.as_str() {
            "sum" => aggregate_numbers(&numbers, |numbers| numbers.iter().sum()),
            "average" | "avg" | "mean" => aggregate_numbers(&numbers, |numbers| {
                numbers.iter().sum::<Decimal>() / Decimal::from(numbers.len())
            }),
            "median" => aggregate_numbers(&numbers, median),
            "min" => aggregate_numbers(&numbers, |numbers| {
                *numbers.iter().min().expect("numbers is non-empty")
            }),
            "max" => aggregate_numbers(&numbers, |numbers| {
                *numbers.iter().max().expect("numbers is non-empty")
            }),
            "count" => Ok(Value::Integer(numbers.len() as i128)),
            _ => Err(EvalError::Incomplete),
        }
    }

    /// Evaluate a line and record its result in the scope for `lineN` references.
    ///
    /// Errors are not returned but folded into the value: a parse error, an incomplete
    /// line or an unknown variable gives an empty `Value::Str` (the line is still being
    /// typed), any other error gives `Value::Err` with the error message.
    ///
    /// # Panics
    ///
    /// On arithmetic overflow outside `**`, see [`Engine::evaluate`].
    ///
    /// # Examples
    ///
    /// ```
    /// use numbr_core::{Engine, Value};
    ///
    /// let mut engine = Engine::new();
    /// assert_eq!(engine.evaluate_line("10"), Value::Integer(10));
    /// assert_eq!(engine.evaluate_line("line1 * 2"), Value::Integer(20));
    /// assert_eq!(engine.evaluate_line("2 +"), Value::Str(String::new()));
    /// assert_eq!(engine.evaluate_line("1 / 0"), Value::Err("Division by zero".into()));
    /// assert_eq!(engine.scope().lines().len(), 4);
    /// ```
    pub fn evaluate_line(&mut self, input: &str) -> Value {
        let result = match self.evaluate(input) {
            Ok(v) => v,
            // Parse errors mean the expression is incomplete/malformed — show nothing.
            Err(
                EvalError::ParseError { .. }
                | EvalError::Incomplete
                | EvalError::UnknownVariable(_),
            ) => Value::Str(String::new()),
            Err(e) => Value::Err(e.to_string()),
        };
        self.interpreter.scope.push_line(result.clone());
        result
    }
}

fn contiguous_result_numbers(lines: &[Value]) -> Vec<Decimal> {
    let mut numbers: Vec<Decimal> = lines
        .iter()
        .rev()
        .skip_while(|value| matches!(value, Value::Str(text) if text.is_empty()))
        .take_while(|value| !matches!(value, Value::Str(text) if text.is_empty()))
        .filter_map(Value::to_decimal)
        .collect();
    numbers.reverse();
    numbers
}

fn aggregate_numbers(
    numbers: &[Decimal],
    f: impl FnOnce(&[Decimal]) -> Decimal,
) -> Result<Value, EvalError> {
    if numbers.is_empty() {
        return Err(EvalError::TypeError(
            "result aggregate has no numeric values".to_owned(),
        ));
    }

    Ok(decimal_value(f(numbers)))
}

fn median(numbers: &[Decimal]) -> Decimal {
    let mut sorted = numbers.to_vec();
    sorted.sort();
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        sorted[mid]
    } else {
        (sorted[mid - 1] + sorted[mid]) / Decimal::from(2)
    }
}

fn decimal_value(value: Decimal) -> Value {
    if value.fract().is_zero() {
        if let Some(integer) = rust_decimal::prelude::ToPrimitive::to_i128(&value) {
            return Value::Integer(integer);
        }
    }
    Value::Decimal(value.normalize())
}

/// Cut `input` at the first `#` outside a string literal.
pub fn strip_comment(input: &str) -> &str {
    let mut escaped = false;
    let mut in_string = false;

    for (idx, ch) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '#' if !in_string => return &input[..idx],
            _ => {}
        }
    }

    input
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
