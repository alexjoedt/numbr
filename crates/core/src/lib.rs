pub mod builtin;
pub mod engine;
pub mod error;
pub mod functions;
pub mod interpreter;
pub mod lexer;
pub mod modbus;
pub mod parser;
pub mod scope;
pub mod units;
pub mod value;

pub use engine::{strip_comment, Engine};
pub use error::{EvalError, FuncError};
pub use scope::Scope;
pub use value::Value;

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::prelude::ToPrimitive;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    fn eval(input: &str) -> Value {
        Engine::new().evaluate(input).expect("evaluation failed")
    }

    fn eval_line(engine: &mut Engine, input: &str) -> Value {
        engine.evaluate_line(input)
    }

    // ── Basic arithmetic ──────────────────────────────────────────────────

    #[test]
    fn test_integer_add() {
        assert_eq!(eval("2 + 3"), Value::Integer(5));
    }

    #[test]
    fn test_integer_sub() {
        assert_eq!(eval("10 - 4"), Value::Integer(6));
    }

    #[test]
    fn test_integer_mul() {
        assert_eq!(eval("6 * 7"), Value::Integer(42));
    }

    #[test]
    fn test_integer_div() {
        assert_eq!(eval("10 / 2"), Value::Integer(5));
    }

    #[test]
    fn test_float_arithmetic() {
        assert_eq!(eval("1.5 + 2.5"), Value::Float(4.0));
    }

    #[test]
    fn test_power() {
        assert_eq!(eval("2 ** 10"), Value::Integer(1024));
    }

    #[test]
    fn test_modulo() {
        assert_eq!(eval("10 mod 3"), Value::Integer(1));
    }

    #[test]
    fn test_unary_neg() {
        assert_eq!(eval("-5"), Value::Integer(-5));
    }

    #[test]
    fn test_operator_precedence() {
        assert_eq!(eval("2 + 3 * 4"), Value::Integer(14));
    }

    #[test]
    fn test_parentheses() {
        assert_eq!(eval("(2 + 3) * 4"), Value::Integer(20));
    }

    #[test]
    fn test_division_by_zero() {
        let result = Engine::new().evaluate("1 / 0");
        assert!(matches!(result, Err(EvalError::DivisionByZero)));
    }

    // ── Variables ─────────────────────────────────────────────────────────

    #[test]
    fn test_variable_assignment_and_reference() {
        let mut e = Engine::new();
        eval_line(&mut e, "price = 42");
        let result = eval_line(&mut e, "price * 3");
        assert_eq!(result, Value::Integer(126));
    }

    #[test]
    fn test_semicolon_sequence() {
        assert_eq!(eval("price = 42; price * 3"), Value::Integer(126));
    }

    // ── Line references ───────────────────────────────────────────────────

    #[test]
    fn test_line_reference() {
        let mut e = Engine::new();
        eval_line(&mut e, "100");
        let result = eval_line(&mut e, "line1 + 10");
        assert_eq!(result, Value::Integer(110));
    }

    // ── Constants ─────────────────────────────────────────────────────────

    #[test]
    fn test_pi_constant() {
        let v = eval("pi");
        assert!(matches!(v, Value::Float(f) if (f - std::f64::consts::PI).abs() < 1e-12));
    }

    // ── Percentages ───────────────────────────────────────────────────────

    #[test]
    fn test_percent_of() {
        // 5% of 200 = 10
        let v = eval("5% of 200");
        // 5% → 0.05 (Decimal); 0.05 * 200 = 10
        assert!(matches!(&v, Value::Decimal(d) if *d == Decimal::from_str("10").unwrap()));
    }

    #[test]
    fn test_standalone_percent() {
        let v = eval("20%");
        assert!(matches!(&v, Value::Decimal(d) if *d == Decimal::from_str("0.20").unwrap()));
    }

    // ── Number systems ────────────────────────────────────────────────────

    #[test]
    fn test_hex_literal() {
        assert_eq!(eval("0xFF"), Value::Integer(255));
    }

    #[test]
    fn test_binary_literal() {
        assert_eq!(eval("0b1010"), Value::Integer(10));
    }

    #[test]
    fn test_octal_literal() {
        assert_eq!(eval("0o17"), Value::Integer(15));
    }

    #[test]
    fn test_convert_to_hex() {
        let v = eval("255 in hex");
        assert_eq!(v, Value::Str("0xFF".to_owned()));
    }

    #[test]
    fn test_convert_to_binary() {
        let v = eval("0xFF in binary");
        assert_eq!(v, Value::Str("0b11111111".to_owned()));
    }

    // ── Bit operations ────────────────────────────────────────────────────

    #[test]
    fn test_bitwise_and() {
        assert_eq!(eval("0xF0 & 0x0F"), Value::Integer(0));
    }

    #[test]
    fn test_bitwise_or() {
        assert_eq!(eval("0xF0 | 0x0F"), Value::Integer(0xFF));
    }

    #[test]
    fn test_shift_left() {
        assert_eq!(eval("1 << 8"), Value::Integer(256));
    }

    #[test]
    fn test_shift_right() {
        assert_eq!(eval("0xFF00 >> 4"), Value::Integer(0x0FF0));
    }

    #[test]
    fn test_bit_not() {
        // ~0i128 = -1 (all bits set)
        assert_eq!(eval("~0"), Value::Integer(-1));
    }

    // ── Bit casts ─────────────────────────────────────────────────────────

    #[test]
    fn test_bit_cast_int8_overflow() {
        // 255 as int8 → -1
        assert_eq!(eval("255 as int8"), Value::Integer(-1));
    }

    #[test]
    fn test_bit_cast_int16() {
        assert_eq!(eval("0xFFFF as int16"), Value::Integer(-1));
    }

    #[test]
    fn test_bit_cast_uint8() {
        assert_eq!(eval("255 as uint8"), Value::Integer(255));
    }

    // ── Built-in functions ────────────────────────────────────────────────

    #[test]
    fn test_sqrt() {
        let v = eval("sqrt(4)");
        assert!(matches!(v, Value::Float(f) if (f - 2.0).abs() < 1e-10));
    }

    #[test]
    fn test_popcount() {
        assert_eq!(eval("popcount(0xFF)"), Value::Integer(8));
    }

    #[test]
    fn test_abs_negative() {
        assert_eq!(eval("abs(-5)"), Value::Integer(5));
    }

    // ── Snapshot tests ────────────────────────────────────────────────────

    #[test]
    fn test_snapshot_developer_conversions() {
        let cases = [
            ("0xFF as uint8", "255"),
            ("0xFF as int8", "-1"),
            ("0b1010 in hex", "0xA"),
            ("popcount(0xFF)", "8"),
            ("255 in binary", "0b11111111"),
        ];
        for (input, expected) in cases {
            let result = Engine::new().evaluate(input).expect("evaluation failed");
            assert_eq!(result.to_string(), expected, "input: {input}");
        }
    }

    // ── M2: Developer functions ───────────────────────────────────────────

    #[test]
    fn test_byteswap16() {
        // 0x1234 → 0x3412
        assert_eq!(eval("byteswap16(0x1234)"), Value::Integer(0x3412));
    }

    #[test]
    fn test_byteswap32() {
        // 0x12345678 → 0x78563412
        assert_eq!(eval("byteswap32(0x12345678)"), Value::Integer(0x78563412));
    }

    // ── M3: Unit conversions ──────────────────────────────────────────────

    #[test]
    fn test_unit_km_to_miles() {
        let v = eval("10 km in miles");
        match v {
            Value::Unit { amount, unit } => {
                assert_eq!(unit, "miles");
                let f = amount.to_f64().unwrap();
                assert!(
                    (f - 6.21371).abs() < 0.001,
                    "expected ~6.21371 miles, got {f}"
                );
            }
            other => panic!("expected Value::Unit, got {other:?}"),
        }
    }

    #[test]
    fn test_unit_kg_to_lbs() {
        let v = eval("1 kg in lbs");
        match v {
            Value::Unit { amount, unit } => {
                assert_eq!(unit, "lbs");
                let f = amount.to_f64().unwrap();
                assert!(
                    (f - 2.20462).abs() < 0.001,
                    "expected ~2.20462 lbs, got {f}"
                );
            }
            other => panic!("expected Value::Unit, got {other:?}"),
        }
    }

    #[test]
    fn test_temperature_celsius_to_fahrenheit() {
        let v = eval("30 celsius in fahrenheit");
        match v {
            Value::Unit { amount, unit } => {
                assert_eq!(unit, "fahrenheit");
                let f = amount.to_f64().unwrap();
                assert!((f - 86.0).abs() < 0.001, "expected 86 fahrenheit, got {f}");
            }
            other => panic!("expected Value::Unit, got {other:?}"),
        }
    }

    #[test]
    fn test_unit_gb_to_mb() {
        // KB/MB/GB/TB use SI decimal (1000-based)
        let v = eval("3.7 GB in MB");
        match v {
            Value::Unit { amount, unit } => {
                assert_eq!(unit, "MB");
                let f = amount.to_f64().unwrap();
                assert!((f - 3700.0).abs() < 0.001, "expected 3700 MB, got {f}");
            }
            other => panic!("expected Value::Unit, got {other:?}"),
        }
    }

    #[test]
    fn test_unit_mb_to_bytes_si() {
        let v = eval("1 MB in bytes");
        match v {
            Value::Unit { amount, unit } => {
                assert_eq!(unit, "bytes");
                let f = amount.to_f64().unwrap();
                assert!(
                    (f - 1_000_000.0).abs() < 0.001,
                    "expected 1000000 bytes, got {f}"
                );
            }
            other => panic!("expected Value::Unit, got {other:?}"),
        }
    }

    #[test]
    fn test_unit_mb_lowercase_alias() {
        // lowercase mb is an alias for MB (megabytes)
        let v = eval("1 mb in bytes");
        match v {
            Value::Unit { amount, unit } => {
                assert_eq!(unit, "bytes");
                let f = amount.to_f64().unwrap();
                assert!(
                    (f - 1_000_000.0).abs() < 0.001,
                    "expected 1000000 bytes, got {f}"
                );
            }
            other => panic!("expected Value::Unit, got {other:?}"),
        }
    }

    #[test]
    fn test_unit_mib_to_bytes_iec() {
        // MiB uses IEC binary (1024-based)
        let v = eval("1 MiB in bytes");
        match v {
            Value::Unit { amount, unit } => {
                assert_eq!(unit, "bytes");
                let f = amount.to_f64().unwrap();
                assert!(
                    (f - 1_048_576.0).abs() < 0.001,
                    "expected 1048576 bytes, got {f}"
                );
            }
            other => panic!("expected Value::Unit, got {other:?}"),
        }
    }

    #[test]
    fn test_unit_gib_to_mib_iec() {
        let v = eval("1 GiB in MiB");
        match v {
            Value::Unit { amount, unit } => {
                assert_eq!(unit, "MiB");
                let f = amount.to_f64().unwrap();
                assert!((f - 1024.0).abs() < 0.001, "expected 1024 MiB, got {f}");
            }
            other => panic!("expected Value::Unit, got {other:?}"),
        }
    }

    #[test]
    fn test_unit_pb_to_tb_si() {
        let v = eval("1 PB in TB");
        match v {
            Value::Unit { amount, unit } => {
                assert_eq!(unit, "TB");
                let f = amount.to_f64().unwrap();
                assert!((f - 1000.0).abs() < 0.001, "expected 1000 TB, got {f}");
            }
            other => panic!("expected Value::Unit, got {other:?}"),
        }
    }

    #[test]
    fn test_unit_mbit_to_kbit() {
        let v = eval("1 mbit in kbit");
        match v {
            Value::Unit { amount, unit } => {
                assert_eq!(unit, "kbit");
                let f = amount.to_f64().unwrap();
                assert!((f - 1000.0).abs() < 0.001, "expected 1000 kbit, got {f}");
            }
            other => panic!("expected Value::Unit, got {other:?}"),
        }
    }

    #[test]
    fn test_unit_cross_system_gib_to_mb() {
        // 1 GiB = 1073741824 B; in MB (1000-based) = 1073741824 / 1_000_000 = 1073.741824
        let v = eval("1 GiB in MB");
        match v {
            Value::Unit { amount, unit } => {
                assert_eq!(unit, "MB");
                let f = amount.to_f64().unwrap();
                assert!(
                    (f - 1_073.741_824).abs() < 0.000_01,
                    "expected ~1073.741824 MB, got {f}"
                );
            }
            other => panic!("expected Value::Unit, got {other:?}"),
        }
    }

    #[test]
    fn test_unit_weeks_to_days() {
        let v = eval("2 weeks in days");
        match v {
            Value::Unit { amount, unit } => {
                assert_eq!(unit, "days");
                let f = amount.to_f64().unwrap();
                assert!((f - 14.0).abs() < 0.001, "expected 14 days, got {f}");
            }
            other => panic!("expected Value::Unit, got {other:?}"),
        }
    }

    // ── M3: Date arithmetic ───────────────────────────────────────────────

    #[test]
    fn test_date_literal() {
        let v = eval("2026-07-04");
        let expected = chrono::NaiveDate::from_ymd_opt(2026, 7, 4).unwrap();
        assert_eq!(v, Value::Date(expected));
    }

    #[test]
    fn test_date_add_weeks() {
        let v = eval("2026-07-04 + 2 weeks");
        let expected = chrono::NaiveDate::from_ymd_opt(2026, 7, 18).unwrap();
        assert_eq!(
            v,
            Value::Date(expected),
            "2 weeks after 2026-07-04 should be 2026-07-18"
        );
    }

    #[test]
    fn test_date_add_days() {
        let v = eval("2026-07-04 + 90 days");
        let expected = chrono::NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
        assert_eq!(v, Value::Date(expected));
    }

    #[test]
    fn test_date_sub_days() {
        let v = eval("2026-07-04 - 10 days");
        let expected = chrono::NaiveDate::from_ymd_opt(2026, 6, 24).unwrap();
        assert_eq!(v, Value::Date(expected));
    }

    #[test]
    fn test_date_diff() {
        // diff(2026-07-04, 2026-01-01) = 184 days
        let v = eval("diff(2026-07-04, 2026-01-01)");
        assert_eq!(v, Value::Integer(184));
    }

    #[test]
    fn test_date_diff_in_weeks() {
        // diff(2026-07-04, 2026-01-01) = 184 days = 26.28... weeks
        let v = eval("diff(2026-07-04, 2026-01-01) in weeks");
        match v {
            Value::Unit { amount, unit } => {
                assert_eq!(unit, "weeks");
                let f = amount.to_f64().unwrap();
                assert!(
                    (f - 184.0 / 7.0).abs() < 0.001,
                    "expected ~{} weeks, got {f}",
                    184.0 / 7.0
                );
            }
            other => panic!("expected Value::Unit, got {other:?}"),
        }
    }

    // ── M4: Modbus conversions ────────────────────────────────────────────

    #[test]
    fn test_modbus_float32_abcd() {
        // 10.5 as IEEE-754 single = 0x4128_0000 → r1=0x4128, r2=0x0000, ABCD (default)
        let v = eval("modbus::float32(0x4128, 0x0000)");
        match v {
            Value::Float(f) => assert!((f - 10.5).abs() < 1e-5, "expected 10.5, got {f}"),
            other => panic!("expected Float, got {other:?}"),
        }
    }

    #[test]
    fn test_modbus_float32_with_explicit_order() {
        let v = eval("modbus::float32(0x4128, 0x0000, ABCD)");
        assert!(matches!(v, Value::Float(f) if (f - 10.5).abs() < 1e-5));
    }

    #[test]
    fn test_modbus_float32_cdab() {
        // CDAB: r1 is low word, r2 is high word — swap the register positions
        let v = eval("modbus::float32(0x0000, 0x4128, CDAB)");
        assert!(matches!(v, Value::Float(f) if (f - 10.5).abs() < 1e-5));
    }

    #[test]
    fn test_modbus_float32le_is_cdab_alias() {
        // float32le(r1, r2) is the same as float32(r1, r2, CDAB)
        let le = eval("modbus::float32le(0x0000, 0x4128)");
        let cdab = eval("modbus::float32(0x0000, 0x4128, CDAB)");
        assert_eq!(le, cdab);
    }

    #[test]
    fn test_modbus_int32_positive() {
        // r1=0x0000, r2=0x0001 → 1
        assert_eq!(eval("modbus::int32(0x0000, 0x0001)"), Value::Integer(1));
    }

    #[test]
    fn test_modbus_int32_negative() {
        // r1=0xFFFF, r2=0xFFFF → 0xFFFF_FFFF as i32 = -1
        assert_eq!(eval("modbus::int32(0xFFFF, 0xFFFF)"), Value::Integer(-1));
    }

    #[test]
    fn test_modbus_uint32_max() {
        // 0xFFFF_FFFF as u32 = 4294967295
        assert_eq!(
            eval("modbus::uint32(0xFFFF, 0xFFFF)"),
            Value::Integer(4_294_967_295)
        );
    }

    #[test]
    fn test_modbus_uint32_cdab() {
        // CDAB: r1=0x0001 (low), r2=0x0000 (high) → 0x0000_0001 = 1
        assert_eq!(
            eval("modbus::uint32(0x0001, 0x0000, CDAB)"),
            Value::Integer(1)
        );
    }

    #[test]
    fn test_modbus_swap_word() {
        // 0x1234 → 0x3412
        assert_eq!(eval("modbus::swap::word(0x1234)"), Value::Integer(0x3412));
    }

    #[test]
    fn test_modbus_swap_byte() {
        // (0x12, 0x34) → 0x3412
        assert_eq!(
            eval("modbus::swap::byte(0x12, 0x34)"),
            Value::Integer(0x3412)
        );
    }

    #[test]
    fn test_modbus_float32_composable_with_sqrt() {
        // sqrt(modbus::float32(r1, r2)) should work (composition)
        // 100.0 as f32 = 0x42C8_0000 → r1=0x42C8, r2=0x0000
        let v = eval("sqrt(modbus::float32(0x42C8, 0x0000))");
        match v {
            Value::Float(f) => assert!((f - 10.0).abs() < 1e-4, "expected 10.0, got {f}"),
            other => panic!("expected Float, got {other:?}"),
        }
    }

    #[test]
    fn test_modbus_snapshot() {
        let cases = [
            ("modbus::int32(0x0000, 0x0001)", "1"),
            ("modbus::uint32(0xFFFF, 0xFFFF)", "4294967295"),
            ("modbus::int32(0xFFFF, 0xFFFF)", "-1"),
            ("modbus::swap::word(0x1234)", "13330"), // 0x3412 = 13330
            ("modbus::swap::byte(0x12, 0x34)", "13330"), // 0x3412 = 13330
        ];
        for (input, expected) in cases {
            let result = Engine::new().evaluate(input).expect("evaluation failed");
            assert_eq!(result.to_string(), expected, "input: {input}");
        }
    }

    // ── Error handling ────────────────────────────────────────────────────

    #[test]
    fn test_parse_error_returns_empty_in_evaluate_line() {
        // Incomplete expression should show nothing, not an error.
        let mut e = Engine::new();
        let v = eval_line(&mut e, "2+");
        assert_eq!(
            v,
            Value::Str(String::new()),
            "incomplete expr must produce empty value"
        );
    }

    #[test]
    fn test_unknown_variable_returns_empty_in_evaluate_line() {
        let v = eval_line(&mut Engine::new(), "undefined_var");
        assert_eq!(v, Value::Str(String::new()));
    }

    #[test]
    fn test_division_by_zero_shows_error() {
        let v = eval_line(&mut Engine::new(), "1 / 0");
        assert!(
            matches!(&v, Value::Err(msg) if msg.contains("Division by zero")),
            "expected division by zero error, got {v:?}",
        );
    }

    #[test]
    fn test_empty_line_returns_empty_string() {
        assert_eq!(eval(""), Value::Str(String::new()));
    }

    #[test]
    fn test_comment_line_returns_empty_string() {
        assert_eq!(eval("# this is a comment"), Value::Str(String::new()));
    }

    #[test]
    fn test_whitespace_only_line_returns_empty_string() {
        assert_eq!(eval("   "), Value::Str(String::new()));
    }

    // ── Session / multi-line ──────────────────────────────────────────────

    #[test]
    fn test_multiple_line_references() {
        let mut e = Engine::new();
        eval_line(&mut e, "10");
        eval_line(&mut e, "20");
        let r = eval_line(&mut e, "line1 + line2");
        assert_eq!(r, Value::Integer(30));
    }

    #[test]
    fn test_variable_survives_blank_line() {
        let mut e = Engine::new();
        eval_line(&mut e, "x = 7");
        eval_line(&mut e, "");
        let r = eval_line(&mut e, "x * 2");
        assert_eq!(r, Value::Integer(14));
    }

    #[test]
    fn test_line_reference_after_comment() {
        let mut e = Engine::new();
        eval_line(&mut e, "42");
        eval_line(&mut e, "# a comment");
        // The comment still advances the line counter.
        let r = eval_line(&mut e, "line1 + 1");
        assert_eq!(r, Value::Integer(43));
    }

    #[test]
    fn test_result_sum() {
        let mut e = Engine::new();
        eval_line(&mut e, "3 + 2");
        eval_line(&mut e, "5 + 5");
        assert_eq!(eval_line(&mut e, "result: sum"), Value::Integer(15));
    }

    #[test]
    fn test_result_average() {
        let mut e = Engine::new();
        eval_line(&mut e, "10");
        eval_line(&mut e, "20");
        assert_eq!(eval_line(&mut e, "result: average"), Value::Integer(15));
    }

    #[test]
    fn test_result_median() {
        let mut e = Engine::new();
        eval_line(&mut e, "30");
        eval_line(&mut e, "10");
        eval_line(&mut e, "20");
        assert_eq!(eval_line(&mut e, "result: median"), Value::Integer(20));

        let mut e = Engine::new();
        eval_line(&mut e, "10");
        eval_line(&mut e, "20");
        eval_line(&mut e, "30");
        eval_line(&mut e, "40");
        assert_eq!(eval_line(&mut e, "result: median"), Value::Integer(25));
    }

    #[test]
    fn test_result_min_max_count() {
        let mut e = Engine::new();
        eval_line(&mut e, "30");
        eval_line(&mut e, "10");
        eval_line(&mut e, "20");
        assert_eq!(eval_line(&mut e, "result: min"), Value::Integer(10));

        let mut e = Engine::new();
        eval_line(&mut e, "30");
        eval_line(&mut e, "10");
        eval_line(&mut e, "20");
        assert_eq!(eval_line(&mut e, "result: max"), Value::Integer(30));

        let mut e = Engine::new();
        eval_line(&mut e, "30");
        eval_line(&mut e, "10");
        eval_line(&mut e, "20");
        assert_eq!(eval_line(&mut e, "result: count"), Value::Integer(3));
    }

    #[test]
    fn test_result_aggregate_only_uses_current_block_after_blank_line() {
        let mut e = Engine::new();
        eval_line(&mut e, "50000");
        eval_line(&mut e, "30000");
        assert_eq!(eval_line(&mut e, "result: sum"), Value::Integer(80000));

        eval_line(&mut e, "");
        eval_line(&mut e, "4000");
        eval_line(&mut e, "3000");
        assert_eq!(eval_line(&mut e, "result: sum"), Value::Integer(7000));
    }

    #[test]
    fn test_result_aggregate_ignores_non_numeric_lines_in_current_block() {
        let mut e = Engine::new();
        eval_line(&mut e, "10");
        eval_line(&mut e, "\"hello\"");
        eval_line(&mut e, "missing_variable");
        assert_eq!(eval_line(&mut e, "result: sum"), Value::Integer(10));
    }

    #[test]
    fn test_result_aggregate_can_be_referenced_later() {
        let mut e = Engine::new();
        eval_line(&mut e, "3 + 2");
        eval_line(&mut e, "5 + 5");
        eval_line(&mut e, "result: sum");
        assert_eq!(eval_line(&mut e, "line3 + 1"), Value::Integer(16));
    }

    #[test]
    fn test_incomplete_result_command_returns_empty_string() {
        let mut e = Engine::new();
        assert_eq!(eval_line(&mut e, "resu"), Value::Str(String::new()));
        assert_eq!(eval_line(&mut e, "result"), Value::Str(String::new()));
        assert_eq!(eval_line(&mut e, "result:"), Value::Str(String::new()));
        assert_eq!(eval_line(&mut e, "result: s"), Value::Str(String::new()));
    }

    // ── Arithmetic edge cases ─────────────────────────────────────────────

    #[test]
    fn test_chained_percent_operations() {
        // 100 + 10% = 110
        let v = eval("100 + 10%");
        assert!(
            matches!(&v, Value::Decimal(d) if *d == rust_decimal::Decimal::from(110)),
            "expected 110, got {v:?}",
        );
    }

    #[test]
    fn test_percent_subtract() {
        // 200 - 25% = 150
        let v = eval("200 - 25%");
        assert!(
            matches!(&v, Value::Decimal(d) if *d == rust_decimal::Decimal::from(150)),
            "expected 150, got {v:?}",
        );
    }

    #[test]
    fn test_negative_power() {
        // (-2) ** 3 = -8
        assert_eq!(eval("(-2) ** 3"), Value::Integer(-8));
    }

    #[test]
    fn test_modulo_negative() {
        // -7 mod 3 in Rust is -1 (truncated division)
        assert_eq!(eval("-7 mod 3"), Value::Integer(-1));
    }

    #[test]
    fn test_bitwise_xor() {
        assert_eq!(eval("0xFF ^ 0x0F"), Value::Integer(0xF0));
    }

    #[test]
    fn test_bit_cast_uint32_large() {
        // 0xFFFF_FFFF as uint32 = 4294967295
        assert_eq!(eval("0xFFFFFFFF as uint32"), Value::Integer(4_294_967_295));
    }

    #[test]
    fn test_bit_cast_int32_negative() {
        // 0xFFFF_FFFF as int32 = -1
        assert_eq!(eval("0xFFFFFFFF as int32"), Value::Integer(-1));
    }

    // ── Scientific functions ──────────────────────────────────────────────

    #[test]
    fn test_sin_zero() {
        let v = eval("sin(0)");
        assert!(matches!(v, Value::Float(f) if f.abs() < 1e-12));
    }

    #[test]
    fn test_cos_zero() {
        let v = eval("cos(0)");
        assert!(matches!(v, Value::Float(f) if (f - 1.0).abs() < 1e-12));
    }

    #[test]
    fn test_floor() {
        assert_eq!(eval("floor(3.9)"), Value::Float(3.0));
    }

    #[test]
    fn test_ceil() {
        assert_eq!(eval("ceil(3.1)"), Value::Float(4.0));
    }

    #[test]
    fn test_round_half_up() {
        assert_eq!(eval("round(2.5)"), Value::Float(3.0));
    }

    #[test]
    fn test_abs_positive() {
        let v = eval("abs(7)");
        assert_eq!(v, Value::Integer(7));
    }

    #[test]
    fn test_exp_zero() {
        let v = eval("exp(0)");
        assert!(matches!(v, Value::Float(f) if (f - 1.0).abs() < 1e-12));
    }

    #[test]
    fn test_ln_e() {
        let v = eval("ln(e)");
        assert!(matches!(v, Value::Float(f) if (f - 1.0).abs() < 1e-10));
    }

    // ── Developer bit functions ───────────────────────────────────────────

    #[test]
    fn test_popcount_zero() {
        assert_eq!(eval("popcount(0)"), Value::Integer(0));
    }

    #[test]
    fn test_clz_one() {
        // clz(1) for a 64-bit value = 63
        assert_eq!(eval("clz(1)"), Value::Integer(63));
    }

    #[test]
    fn test_ctz_eight() {
        // ctz(8) = ctz(0b1000) = 3
        assert_eq!(eval("ctz(8)"), Value::Integer(3));
    }

    // ── Units: additional conversions ─────────────────────────────────────

    #[test]
    fn test_unit_metres_to_feet() {
        let v = eval("1 meters in feet");
        match v {
            Value::Unit { amount, unit } => {
                assert_eq!(unit, "feet");
                let f = amount.to_f64().unwrap();
                assert!((f - 3.28084).abs() < 0.001, "expected ~3.28084 ft, got {f}");
            }
            other => panic!("expected Value::Unit, got {other:?}"),
        }
    }

    #[test]
    fn test_unit_celsius_to_kelvin() {
        let v = eval("0 celsius in kelvin");
        match v {
            Value::Unit { amount, unit } => {
                assert_eq!(unit, "kelvin");
                let f = amount.to_f64().unwrap();
                assert!((f - 273.15).abs() < 0.01, "expected 273.15 K, got {f}");
            }
            other => panic!("expected Value::Unit, got {other:?}"),
        }
    }

    #[test]
    fn test_unit_grams_to_kg() {
        let v = eval("1000 grams in kg");
        match v {
            Value::Unit { amount, unit } => {
                assert_eq!(unit, "kg");
                let f = amount.to_f64().unwrap();
                assert!((f - 1.0).abs() < 0.001, "expected 1 kg, got {f}");
            }
            other => panic!("expected Value::Unit, got {other:?}"),
        }
    }

    #[test]
    fn test_unit_hours_to_minutes() {
        let v = eval("2 hours in min");
        match v {
            Value::Unit { amount, unit } => {
                assert_eq!(unit, "min");
                let f = amount.to_f64().unwrap();
                assert!((f - 120.0).abs() < 0.001, "expected 120 min, got {f}");
            }
            other => panic!("expected Value::Unit, got {other:?}"),
        }
    }

    #[test]
    fn test_short_unit_aliases_are_known_without_lexer_duplication() {
        let v = eval("1 m in feet");
        match v {
            Value::Unit { amount, unit } => {
                assert_eq!(unit, "feet");
                let f = amount.to_f64().unwrap();
                assert!((f - 3.28084).abs() < 0.001, "expected ~3.28084 ft, got {f}");
            }
            other => panic!("expected Value::Unit, got {other:?}"),
        }
    }

    #[test]
    fn test_inline_comment_is_ignored() {
        assert_eq!(eval("2 + 2 # note"), Value::Integer(4));
    }

    #[test]
    fn test_hash_inside_string_is_not_comment() {
        assert_eq!(eval(r#""a#b""#), Value::Str("a#b".to_owned()));
    }

    #[test]
    fn test_invalid_character_is_parse_error() {
        let result = Engine::new().evaluate("@");
        assert!(matches!(result, Err(EvalError::ParseError { .. })));
    }

    #[test]
    fn test_float_promotion_does_not_coerce_non_numeric_to_zero() {
        let result = Engine::new().evaluate(r#""abc" + 1.5"#);
        assert!(matches!(result, Err(EvalError::TypeError(_))));
    }

    #[test]
    fn test_shift_count_out_of_range_is_error() {
        let result = Engine::new().evaluate("1 << 200");
        assert!(matches!(result, Err(EvalError::TypeError(_))));
    }

    #[test]
    fn test_large_integer_exponent_is_error() {
        let result = Engine::new().evaluate("2 ** 4294967296");
        assert!(matches!(result, Err(EvalError::TypeError(_))));
    }

    #[test]
    fn test_float_modulo_by_zero_is_error() {
        let result = Engine::new().evaluate("1.5 mod 0.0");
        assert!(matches!(result, Err(EvalError::DivisionByZero)));
    }

    // ── Snapshot coverage across all milestones ───────────────────────────

    #[test]
    fn test_snapshot_arithmetic() {
        let cases = [
            ("2 + 3", "5"),
            ("10 - 4", "6"),
            ("6 * 7", "42"),
            ("10 / 2", "5"),
            ("2 ** 10", "1024"),
            ("10 mod 3", "1"),
            ("-5", "-5"),
            ("2 + 3 * 4", "14"),
            ("(2 + 3) * 4", "20"),
        ];
        for (input, expected) in cases {
            let result = Engine::new().evaluate(input).expect("evaluation failed");
            assert_eq!(result.to_string(), expected, "input: {input}");
        }
    }

    #[test]
    fn test_snapshot_number_systems() {
        let cases = [
            ("0xFF", "255"),
            ("0b1010", "10"),
            ("0o17", "15"),
            ("255 in hex", "0xFF"),
            ("255 in binary", "0b11111111"),
            ("255 in octal", "0o377"),
            ("0b1010 in hex", "0xA"),
            ("0xFF as uint8", "255"),
            ("0xFF as int8", "-1"),
            ("0xFFFF as int16", "-1"),
            ("0xFFFF as uint16", "65535"),
            ("popcount(0xFF)", "8"),
        ];
        for (input, expected) in cases {
            let result = Engine::new().evaluate(input).expect("evaluation failed");
            assert_eq!(result.to_string(), expected, "input: {input}");
        }
    }

    #[test]
    fn test_snapshot_units() {
        // We test the display string; exact amounts already covered by dedicated tests.
        let cases = [
            ("1 km in meters", "1000 meters"),
            ("100 cm in meters", "1 meters"),
            ("1000 grams in kg", "1 kg"),
        ];
        for (input, expected) in cases {
            let result = Engine::new().evaluate(input).expect("evaluation failed");
            assert_eq!(result.to_string(), expected, "input: {input}");
        }
    }
}
