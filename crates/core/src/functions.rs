use crate::error::FuncError;
use crate::value::Value;

/// Dependency-injected function dispatch.
/// `core` only knows this trait — never the concrete implementation.
pub trait FunctionProvider: Send + Sync {
    /// Whether this provider implements the function `name`.
    fn provides(&self, name: &str) -> bool;

    /// Call `name` with already evaluated arguments.
    ///
    /// # Errors
    ///
    /// [`FuncError::NotFound`] when `name` is not provided, [`FuncError::ArgCount`] for a
    /// wrong number of arguments, [`FuncError::ArgType`] for an argument of the wrong kind
    /// or out of range.
    fn call(&self, name: &str, args: &[Value]) -> Result<Value, FuncError>;
}
