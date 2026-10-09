use crate::value::Value;
use std::collections::HashMap;

/// Per-evaluation context: named variables + per-line results.
#[derive(Debug, Default, Clone)]
pub struct Scope {
    vars: HashMap<String, Value>,
    /// Indexed by 1-based line number
    lines: Vec<Value>,
}

impl Scope {
    /// Create an empty scope.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind `name` to `value`, replacing any previous binding.
    pub fn set_var(&mut self, name: &str, value: Value) {
        self.vars.insert(name.to_owned(), value);
    }

    /// Value bound to `name`, if any. Does not resolve `lineN`; see [`Scope::resolve`].
    pub fn get_var(&self, name: &str) -> Option<&Value> {
        self.vars.get(name)
    }

    /// Record the result of a line. Returns 1-based index.
    pub fn push_line(&mut self, value: Value) -> usize {
        self.lines.push(value);
        self.lines.len()
    }

    /// All recorded line results, in evaluation order.
    pub fn lines(&self) -> &[Value] {
        &self.lines
    }

    /// Result of line `n` (1-based), as referenced by `line1` … `lineN`.
    pub fn get_line(&self, n: usize) -> Option<&Value> {
        self.lines.get(n.saturating_sub(1))
    }

    /// Look up `name`: first check line refs, then variables.
    pub fn resolve(&self, name: &str) -> Option<Value> {
        // line1 .. lineN
        if let Some(rest) = name.strip_prefix("line") {
            if let Ok(n) = rest.parse::<usize>() {
                return self.get_line(n).cloned();
            }
        }
        self.get_var(name).cloned()
    }
}
