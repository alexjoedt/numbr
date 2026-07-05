use crate::error::FuncError;
use crate::value::Value;

/// Dependency-injected function dispatch.
/// `core` only knows this trait — never the concrete implementation.
pub trait FunctionProvider: Send + Sync {
    fn provides(&self, name: &str) -> bool;
    fn call(&self, name: &str, args: &[Value]) -> Result<Value, FuncError>;
}
