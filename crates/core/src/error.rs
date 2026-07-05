use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum EvalError {
    #[error("Division by zero")]
    DivisionByZero,
    #[error("Unknown variable '{0}'")]
    UnknownVariable(String),
    #[error("Overflow: {0} exceeds {1}-bit width")]
    Overflow(i128, usize),
    #[error("Parse error at position {pos}: {message}")]
    ParseError { pos: usize, message: String },
    /// Returned when the input is syntactically incomplete (e.g. trailing operator).
    #[error("Incomplete expression")]
    Incomplete,
    #[error("Type error: {0}")]
    TypeError(String),
    #[error("Unknown unit '{0}'")]
    UnknownUnit(String),
    #[error("Function error: {0}")]
    FuncError(#[from] FuncError),
}

#[derive(Error, Debug, Clone, PartialEq)]
pub enum FuncError {
    #[error("Function '{0}' not found")]
    NotFound(String),
    #[error("Wrong argument count for '{name}': got {got}, want {want}")]
    ArgCount {
        name: String,
        got: usize,
        want: usize,
    },
    #[error("Invalid argument type for '{name}': {details}")]
    ArgType { name: String, details: String },
}
