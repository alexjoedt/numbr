//! Modbus register conversion functions.
//!
//! Implements calculation-only Modbus conversions (no live communication):
//! - `modbus::float32` / `modbus::float32le` — IEEE-754 float from two 16-bit registers
//! - `modbus::int32`  — signed 32-bit integer from two registers
//! - `modbus::uint32` — unsigned 32-bit integer from two registers
//! - `modbus::swap::word` — byte-swap a single 16-bit register
//! - `modbus::swap::byte` — combine two bytes in reversed order
//!
//! ## Register-order formats
//!
//! | Format | Description                                  |
//! |--------|----------------------------------------------|
//! | `ABCD` | Big-endian (default): r1 is high word        |
//! | `CDAB` | Word-swapped: r1 is low word                 |
//! | `BADC` | Byte-swapped within each 16-bit word         |
//! | `DCBA` | Full little-endian (word-swap + byte-swap)   |
//!
//! Pass the order as either a bare identifier (`CDAB`) or a string literal (`"CDAB"`)
//! — both are accepted; bare identifiers are resolved as `Value::Str` constants by the
//! interpreter before the function is called.

use crate::error::FuncError;
use crate::functions::FunctionProvider;
use crate::value::Value;

/// Provides all `modbus::*` built-in functions.
pub struct ModbusFunctions;

/// Byte/word ordering for 32-bit Modbus multi-register reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegisterOrder {
    /// Big-endian (default): bytes A B C D — r1 is high word, r2 is low word.
    Abcd,
    /// Word-swapped: bytes C D A B — r1 is low word, r2 is high word.
    Cdab,
    /// Byte-swapped within each 16-bit word: bytes B A D C.
    Badc,
    /// Full little-endian / double-swap: bytes D C B A.
    Dcba,
}

impl RegisterOrder {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "ABCD" => Some(Self::Abcd),
            "CDAB" => Some(Self::Cdab),
            "BADC" => Some(Self::Badc),
            "DCBA" => Some(Self::Dcba),
            _ => None,
        }
    }
}

/// Combine two 16-bit Modbus registers into a single 32-bit raw value using the
/// specified register/byte order.
fn combine_registers(r1: u16, r2: u16, order: RegisterOrder) -> u32 {
    match order {
        // ABCD: r1 occupies the high 16 bits, r2 the low 16 bits.
        RegisterOrder::Abcd => ((r1 as u32) << 16) | (r2 as u32),
        // CDAB: r2 occupies the high 16 bits, r1 the low 16 bits.
        RegisterOrder::Cdab => ((r2 as u32) << 16) | (r1 as u32),
        // BADC: each 16-bit word is byte-swapped before combining (r1 still high).
        RegisterOrder::Badc => ((r1.swap_bytes() as u32) << 16) | (r2.swap_bytes() as u32),
        // DCBA: word-swap + byte-swap: r2 (byte-swapped) in high, r1 (byte-swapped) in low.
        RegisterOrder::Dcba => ((r2.swap_bytes() as u32) << 16) | (r1.swap_bytes() as u32),
    }
}

// ── Argument helpers ──────────────────────────────────────────────────────────

/// Extract a 16-bit register value (0–65535) from `args[i]`.
fn reg_arg(name: &str, args: &[Value], i: usize) -> Result<u16, FuncError> {
    args[i]
        .to_integer()
        .and_then(|n| {
            if (0..=u16::MAX as i128).contains(&n) {
                Some(n as u16)
            } else {
                None
            }
        })
        .ok_or_else(|| FuncError::ArgType {
            name: name.to_owned(),
            details: format!(
                "argument {} must be a 16-bit register value (0-65535)",
                i + 1
            ),
        })
}

/// Extract a byte value (0-255) from `args[i]`.
fn byte_arg(name: &str, args: &[Value], i: usize) -> Result<u8, FuncError> {
    args[i]
        .to_integer()
        .and_then(|n| {
            if (0..=u8::MAX as i128).contains(&n) {
                Some(n as u8)
            } else {
                None
            }
        })
        .ok_or_else(|| FuncError::ArgType {
            name: name.to_owned(),
            details: format!("argument {} must be a byte value (0-255)", i + 1),
        })
}

/// Extract a register order string from `args[i]`.
fn order_arg(name: &str, args: &[Value], i: usize) -> Result<RegisterOrder, FuncError> {
    let s = match &args[i] {
        Value::Str(s) => s.as_str(),
        _ => {
            return Err(FuncError::ArgType {
                name: name.to_owned(),
                details: format!(
                    "argument {} must be a register order string (ABCD, CDAB, BADC, or DCBA)",
                    i + 1
                ),
            });
        }
    };
    RegisterOrder::from_str(s).ok_or_else(|| FuncError::ArgType {
        name: name.to_owned(),
        details: format!("unknown register order '{s}'; expected ABCD, CDAB, BADC, or DCBA"),
    })
}

/// Parse two register arguments plus an optional order (2 or 3 args total).
fn two_reg_args(name: &str, args: &[Value]) -> Result<(u16, u16, RegisterOrder), FuncError> {
    match args.len() {
        2 => Ok((
            reg_arg(name, args, 0)?,
            reg_arg(name, args, 1)?,
            RegisterOrder::Abcd,
        )),
        3 => Ok((
            reg_arg(name, args, 0)?,
            reg_arg(name, args, 1)?,
            order_arg(name, args, 2)?,
        )),
        n => Err(FuncError::ArgType {
            name: name.to_owned(),
            details: format!("expected 2 or 3 arguments, got {n}"),
        }),
    }
}

// ── FunctionProvider impl ─────────────────────────────────────────────────────

impl FunctionProvider for ModbusFunctions {
    fn provides(&self, name: &str) -> bool {
        matches!(
            name,
            "modbus_float32"
                | "modbus_float32le"
                | "modbus_int32"
                | "modbus_uint32"
                | "modbus_swap_word"
                | "modbus_swap_byte"
        )
    }

    fn call(&self, name: &str, args: &[Value]) -> Result<Value, FuncError> {
        match name {
            // ── Float 32-bit ────────────────────────────────────────────────
            "modbus_float32" => {
                let (r1, r2, order) = two_reg_args(name, args)?;
                let raw = combine_registers(r1, r2, order);
                Ok(Value::Float(f32::from_bits(raw) as f64))
            }

            // Alias: float32le = float32 with CDAB (little-endian word order).
            "modbus_float32le" => {
                if args.len() != 2 {
                    return Err(FuncError::ArgCount {
                        name: name.to_owned(),
                        got: args.len(),
                        want: 2,
                    });
                }
                let raw = combine_registers(
                    reg_arg(name, args, 0)?,
                    reg_arg(name, args, 1)?,
                    RegisterOrder::Cdab,
                );
                Ok(Value::Float(f32::from_bits(raw) as f64))
            }

            // ── Signed 32-bit integer ───────────────────────────────────────
            "modbus_int32" => {
                let (r1, r2, order) = two_reg_args(name, args)?;
                let raw = combine_registers(r1, r2, order);
                Ok(Value::Integer(raw as i32 as i128))
            }

            // ── Unsigned 32-bit integer ─────────────────────────────────────
            "modbus_uint32" => {
                let (r1, r2, order) = two_reg_args(name, args)?;
                let raw = combine_registers(r1, r2, order);
                Ok(Value::Integer(raw as i128))
            }

            // ── Byte-swap a single 16-bit register ─────────────────────────
            "modbus_swap_word" => {
                if args.len() != 1 {
                    return Err(FuncError::ArgCount {
                        name: name.to_owned(),
                        got: args.len(),
                        want: 1,
                    });
                }
                let reg = reg_arg(name, args, 0)?;
                Ok(Value::Integer(reg.swap_bytes() as i128))
            }

            // ── Combine two bytes in reversed order ─────────────────────────
            "modbus_swap_byte" => {
                if args.len() != 2 {
                    return Err(FuncError::ArgCount {
                        name: name.to_owned(),
                        got: args.len(),
                        want: 2,
                    });
                }
                let b1 = byte_arg(name, args, 0)?;
                let b2 = byte_arg(name, args, 1)?;
                // Swap: b2 becomes the high byte, b1 the low byte.
                let result = ((b2 as u16) << 8) | (b1 as u16);
                Ok(Value::Integer(result as i128))
            }

            _ => Err(FuncError::NotFound(name.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, args: &[Value]) -> Value {
        ModbusFunctions.call(name, args).expect("call failed")
    }

    // ── combine_registers ─────────────────────────────────────────────────

    #[test]
    fn combine_abcd() {
        // ABCD: r1 = high word, r2 = low word
        assert_eq!(
            combine_registers(0x4128, 0x0000, RegisterOrder::Abcd),
            0x4128_0000
        );
    }

    #[test]
    fn combine_cdab() {
        // CDAB: r1 = low word, r2 = high word
        assert_eq!(
            combine_registers(0x0000, 0x4128, RegisterOrder::Cdab),
            0x4128_0000
        );
    }

    #[test]
    fn combine_badc() {
        // BADC: byte-swap each word; r1=0x4128 → 0x2841, r2=0x0000 → 0x0000
        // combined = 0x2841_0000
        assert_eq!(
            combine_registers(0x4128, 0x0000, RegisterOrder::Badc),
            0x2841_0000
        );
    }

    #[test]
    fn combine_dcba() {
        // DCBA: r2 byte-swapped goes high, r1 byte-swapped goes low
        // r1=0x4128 → 0x2841, r2=0x0000 → 0x0000
        // combined = 0x0000_2841
        assert_eq!(
            combine_registers(0x4128, 0x0000, RegisterOrder::Dcba),
            0x0000_2841
        );
    }

    // ── modbus_float32 ────────────────────────────────────────────────────

    #[test]
    fn float32_abcd_default() {
        // 10.5 as IEEE-754 single = 0x4128_0000 → r1=0x4128, r2=0x0000
        let v = call(
            "modbus_float32",
            &[Value::Integer(0x4128), Value::Integer(0x0000)],
        );
        let f = match v {
            Value::Float(f) => f,
            other => panic!("expected Float, got {other:?}"),
        };
        assert!((f - 10.5_f64).abs() < 1e-5, "expected 10.5, got {f}");
    }

    #[test]
    fn float32_with_explicit_abcd_order() {
        let v = call(
            "modbus_float32",
            &[
                Value::Integer(0x4128),
                Value::Integer(0x0000),
                Value::Str("ABCD".into()),
            ],
        );
        assert!(matches!(v, Value::Float(f) if (f - 10.5).abs() < 1e-5));
    }

    #[test]
    fn float32_cdab_word_swap() {
        // CDAB: r1=0x0000 is low word, r2=0x4128 is high word → same 0x4128_0000 = 10.5
        let v = call(
            "modbus_float32",
            &[
                Value::Integer(0x0000),
                Value::Integer(0x4128),
                Value::Str("CDAB".into()),
            ],
        );
        assert!(matches!(v, Value::Float(f) if (f - 10.5).abs() < 1e-5));
    }

    #[test]
    fn float32le_is_cdab_alias() {
        // float32le(r1, r2) = float32(r1, r2, CDAB)
        let le = call(
            "modbus_float32le",
            &[Value::Integer(0x0000), Value::Integer(0x4128)],
        );
        let cdab = call(
            "modbus_float32",
            &[
                Value::Integer(0x0000),
                Value::Integer(0x4128),
                Value::Str("CDAB".into()),
            ],
        );
        assert_eq!(le, cdab);
    }

    // ── modbus_int32 ──────────────────────────────────────────────────────

    #[test]
    fn int32_positive() {
        // r1=0x0000, r2=0x0001 → 0x0000_0001 = 1
        let v = call(
            "modbus_int32",
            &[Value::Integer(0x0000), Value::Integer(0x0001)],
        );
        assert_eq!(v, Value::Integer(1));
    }

    #[test]
    fn int32_negative_twos_complement() {
        // r1=0xFFFF, r2=0xFFFF → 0xFFFF_FFFF as i32 = -1
        let v = call(
            "modbus_int32",
            &[Value::Integer(0xFFFF), Value::Integer(0xFFFF)],
        );
        assert_eq!(v, Value::Integer(-1));
    }

    #[test]
    fn int32_cdab_order() {
        // CDAB: r1=0x0001, r2=0x0000 → combined = 0x0000_0001 = 1
        let v = call(
            "modbus_int32",
            &[
                Value::Integer(0x0001),
                Value::Integer(0x0000),
                Value::Str("CDAB".into()),
            ],
        );
        assert_eq!(v, Value::Integer(1));
    }

    // ── modbus_uint32 ─────────────────────────────────────────────────────

    #[test]
    fn uint32_max_registers() {
        // 0xFFFF_FFFF as u32 = 4294967295
        let v = call(
            "modbus_uint32",
            &[Value::Integer(0xFFFF), Value::Integer(0xFFFF)],
        );
        assert_eq!(v, Value::Integer(4_294_967_295));
    }

    #[test]
    fn uint32_differs_from_int32_at_high_values() {
        let i = call(
            "modbus_int32",
            &[Value::Integer(0xFFFF), Value::Integer(0xFFFF)],
        );
        let u = call(
            "modbus_uint32",
            &[Value::Integer(0xFFFF), Value::Integer(0xFFFF)],
        );
        assert_eq!(i, Value::Integer(-1));
        assert_eq!(u, Value::Integer(4_294_967_295));
    }

    // ── modbus_swap_word ──────────────────────────────────────────────────

    #[test]
    fn swap_word_basic() {
        // 0x1234 → 0x3412
        let v = call("modbus_swap_word", &[Value::Integer(0x1234)]);
        assert_eq!(v, Value::Integer(0x3412));
    }

    #[test]
    fn swap_word_identity_for_symmetric() {
        let v = call("modbus_swap_word", &[Value::Integer(0xAAAA)]);
        assert_eq!(v, Value::Integer(0xAAAA));
    }

    // ── modbus_swap_byte ──────────────────────────────────────────────────

    #[test]
    fn swap_byte_basic() {
        // (0x12, 0x34) → 0x3412 (b2 high, b1 low)
        let v = call(
            "modbus_swap_byte",
            &[Value::Integer(0x12), Value::Integer(0x34)],
        );
        assert_eq!(v, Value::Integer(0x3412));
    }

    #[test]
    fn swap_byte_reversed() {
        // (0x34, 0x12) → 0x1234
        let v = call(
            "modbus_swap_byte",
            &[Value::Integer(0x34), Value::Integer(0x12)],
        );
        assert_eq!(v, Value::Integer(0x1234));
    }
}
