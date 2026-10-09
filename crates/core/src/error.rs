use thiserror::Error;

/// Error returned when a line cannot be evaluated.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum EvalError {
    /// Division or `mod` by zero.
    #[error("Division by zero")]
    DivisionByZero,
    /// An identifier that is neither a variable, a `lineN` reference nor a constant.
    #[error("Unknown variable '{0}'")]
    UnknownVariable(String),
    /// A value does not fit the given bit width. Not returned by the current evaluator;
    /// `as` casts wrap instead.
    #[error("Overflow: {0} exceeds {1}-bit width")]
    Overflow(i128, usize),
    /// The input is not valid syntax.
    #[error("Parse error at position {pos}: {message}")]
    ParseError {
        /// Byte offset where parsing failed, into the string handed to the parser.
        /// [`Engine`](crate::Engine) strips the `#` comment and trims the line first,
        /// so leading whitespace in the raw input shifts the offset.
        pos: usize,
        /// What the parser expected or found.
        message: String,
    },
    /// An unknown `result:` aggregate. A trailing operator is a [`EvalError::ParseError`].
    #[error("Incomplete expression")]
    Incomplete,
    /// An operator or conversion was applied to values of the wrong kind.
    #[error("Type error: {0}")]
    TypeError(String),
    /// A unit name that no conversion table knows.
    #[error("Unknown unit '{0}'")]
    UnknownUnit(String),
    /// A function call failed.
    #[error("Function error: {0}")]
    FuncError(#[from] FuncError),
}

/// Error returned by a [`FunctionProvider`](crate::functions::FunctionProvider) call.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum FuncError {
    /// No provider knows a function of this name.
    #[error("Function '{0}' not found")]
    NotFound(String),
    /// The function was called with the wrong number of arguments.
    #[error("Wrong argument count for '{name}': got {got}, want {want}")]
    ArgCount {
        /// Function name.
        name: String,
        /// Number of arguments passed.
        got: usize,
        /// Number of arguments expected.
        want: usize,
    },
    /// An argument has a type or value the function does not accept.
    #[error("Invalid argument type for '{name}': {details}")]
    ArgType {
        /// Function name.
        name: String,
        /// What was wrong with the argument.
        details: String,
    },
}
