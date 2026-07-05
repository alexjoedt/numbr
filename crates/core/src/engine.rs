use crate::builtin::BuiltinFunctions;
use crate::error::EvalError;
use crate::functions::FunctionProvider;
use crate::interpreter::Interpreter;
use crate::modbus::ModbusFunctions;
use crate::parser;
use crate::scope::Scope;
use crate::value::Value;
use rust_decimal::Decimal;

/// The public API of the `numbr-core` engine.
pub struct Engine {
    interpreter: Interpreter,
}

impl Engine {
    /// Create a new engine with built-in functions only.
    pub fn new() -> Self {
        Self::with_providers(vec![Box::new(BuiltinFunctions), Box::new(ModbusFunctions)])
    }

    /// Create a new engine with custom providers (for dependency injection).
    pub fn with_providers(providers: Vec<Box<dyn FunctionProvider>>) -> Self {
        Self {
            interpreter: Interpreter::new(providers),
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
        }
    }

    /// Snapshot of the current scope (variables + line results recorded so far).
    pub fn scope(&self) -> &Scope {
        &self.interpreter.scope
    }

    /// Evaluate a single line. Returns the result value or an error.
    /// **Never panics.** Errors propagate as `Err(EvalError)`.
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
        let ast = parser::parse(trimmed)?;
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
