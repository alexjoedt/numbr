use crate::error::FuncError;
use crate::functions::FunctionProvider;
use crate::value::Value;
use rust_decimal::prelude::*;

pub struct BuiltinFunctions;

impl FunctionProvider for BuiltinFunctions {
    fn provides(&self, name: &str) -> bool {
        matches!(
            name,
            "sqrt"
                | "sin"
                | "cos"
                | "tan"
                | "log"
                | "ln"
                | "log10"
                | "exp"
                | "abs"
                | "floor"
                | "ceil"
                | "round"
                | "popcount"
                | "clz"
                | "ctz"
                | "byteswap16"
                | "byteswap32"
                | "diff"
        )
    }

    fn call(&self, name: &str, args: &[Value]) -> Result<Value, FuncError> {
        let arg_count = |n: usize| -> Result<(), FuncError> {
            if args.len() != n {
                Err(FuncError::ArgCount {
                    name: name.to_owned(),
                    got: args.len(),
                    want: n,
                })
            } else {
                Ok(())
            }
        };

        let float_arg = |i: usize| -> Result<f64, FuncError> {
            args[i].to_f64().ok_or_else(|| FuncError::ArgType {
                name: name.to_owned(),
                details: format!("argument {i} must be numeric"),
            })
        };

        let int_arg = |i: usize| -> Result<i128, FuncError> {
            args[i].to_integer().ok_or_else(|| FuncError::ArgType {
                name: name.to_owned(),
                details: format!("argument {i} must be integer"),
            })
        };

        match name {
            "sqrt" => {
                arg_count(1)?;
                Ok(Value::Float(float_arg(0)?.sqrt()))
            }
            "sin" => {
                arg_count(1)?;
                Ok(Value::Float(float_arg(0)?.sin()))
            }
            "cos" => {
                arg_count(1)?;
                Ok(Value::Float(float_arg(0)?.cos()))
            }
            "tan" => {
                arg_count(1)?;
                Ok(Value::Float(float_arg(0)?.tan()))
            }
            "log" | "ln" => {
                arg_count(1)?;
                Ok(Value::Float(float_arg(0)?.ln()))
            }
            "log10" => {
                arg_count(1)?;
                Ok(Value::Float(float_arg(0)?.log10()))
            }
            "exp" => {
                arg_count(1)?;
                Ok(Value::Float(float_arg(0)?.exp()))
            }

            "abs" => {
                arg_count(1)?;
                match &args[0] {
                    Value::Integer(i) => Ok(Value::Integer(i.abs())),
                    Value::Decimal(d) => Ok(Value::Decimal(d.abs())),
                    _ => Ok(Value::Float(float_arg(0)?.abs())),
                }
            }
            "floor" => {
                arg_count(1)?;
                match &args[0] {
                    Value::Decimal(d) => Ok(Value::Decimal(d.floor())),
                    _ => Ok(Value::Float(float_arg(0)?.floor())),
                }
            }
            "ceil" => {
                arg_count(1)?;
                match &args[0] {
                    Value::Decimal(d) => Ok(Value::Decimal(d.ceil())),
                    _ => Ok(Value::Float(float_arg(0)?.ceil())),
                }
            }
            "round" => {
                arg_count(1)?;
                match &args[0] {
                    Value::Decimal(d) => Ok(Value::Decimal(d.round())),
                    _ => Ok(Value::Float(float_arg(0)?.round())),
                }
            }

            "popcount" => {
                arg_count(1)?;
                let n = int_arg(0)? as u64;
                Ok(Value::Integer(n.count_ones() as i128))
            }
            "clz" => {
                arg_count(1)?;
                let n = int_arg(0)? as u64;
                Ok(Value::Integer(n.leading_zeros() as i128))
            }
            "ctz" => {
                arg_count(1)?;
                let n = int_arg(0)? as u64;
                Ok(Value::Integer(n.trailing_zeros() as i128))
            }

            "byteswap16" => {
                arg_count(1)?;
                let n = int_arg(0)? as u16;
                Ok(Value::Integer(n.swap_bytes() as i128))
            }
            "byteswap32" => {
                arg_count(1)?;
                let n = int_arg(0)? as u32;
                Ok(Value::Integer(n.swap_bytes() as i128))
            }

            "diff" => {
                arg_count(2)?;
                match (&args[0], &args[1]) {
                    (Value::Date(d1), Value::Date(d2)) => {
                        let days = (*d1 - *d2).num_days();
                        Ok(Value::Integer(days.into()))
                    }
                    (Value::DateTime(dt1), Value::DateTime(dt2)) => {
                        let secs = (*dt1 - *dt2).num_seconds();
                        Ok(Value::Integer(secs.into()))
                    }
                    _ => Err(FuncError::ArgType {
                        name: name.to_owned(),
                        details: "both arguments must be dates or datetimes".into(),
                    }),
                }
            }

            _ => Err(FuncError::NotFound(name.to_owned())),
        }
    }
}
