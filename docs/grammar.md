# Expression grammar

This is a reference for the numbr expression language as the parser implements it today. It
covers the tokens, the operator precedence, and how a multi-line document is evaluated.

Most examples on this page are pinned by `test_table_grammar_doc` and
`test_grammar_doc_evaluation_model` in `crates/core/src/lib.rs`. Error examples are checked
by error kind, not by message text, and prose without an example is not tested. If you change
the grammar, the tests show which examples no longer hold.

The code lives in `crates/core/src`:

| File | Owns |
|---|---|
| `engine.rs` | Per-line entry point, comment stripping, `result:` commands |
| `lexer.rs` | Token set (logos) |
| `parser.rs` | Recursive-descent parser, one function per precedence level |
| `interpreter.rs` | Evaluation, constants, `X + N%` rule |
| `units.rs` | Unit names and conversions |
| `builtin.rs`, `modbus.rs` | Functions |

## Precedence

Loosest first. Each level is one function in `crates/core/src/parser.rs`; it parses its
operands with the next level down the table.

| Level | Operators | Associativity | Example | Result |
|---|---|---|---|---|
| Sequence (`parse_sequence`) | `a; b` | left, all parts evaluated, last one is the value | `x = 2; x * 3` | `6` |
| Assignment (`parse_assign`) | `name = expr` | none, `a = b = 1` is a parse error | `x = 255 in hex` | `0xFF` |
| Conversion (`parse_convert`) | `expr in <target>`, `expr as <type>` | left, chainable | `300 as uint8 in hex` | `0x2C` |
| Percent of (`parse_percent_of`) | `X% of Y` | none | `5% of 200` | `10.00` |
| Additive (`parse_additive`) | `+`, `-` | left | `10 - 4 - 3` | `3` |
| Multiplicative (`parse_multiplicative`) | `*`, `/`, `mod`, `&`, `\|`, `^`, `<<`, `>>` | left | `1 \| 2 * 3` | `9` |
| Power (`parse_power`) | `**` | right | `2 ** 3 ** 2` | `512` |
| Prefix (`parse_unary`) | `-`, `~` | prefix, nestable | `-~0` | `1` |
| Postfix (`parse_postfix`) | `%`, unit name | at most one per operand | `2 * 3 km` | `6 km` |
| Primary (`parse_primary`) | literals, identifiers, `f(args)`, `( ... )` | | `(2 + 3) * 4` | `20` |

Conversion binds looser than `+`, so a conversion applies to the whole expression on its left:
`2 + 3 * 4 in hex` is `(2 + 3 * 4) in hex` = `0xE`.

`in` accepts `hex`/`hexadecimal`, `binary`/`bin`, `octal`/`oct`, `decimal`/`dec`, or any unit
name (`1 mile in km` = `1.609344 km`, `3 km in m` = `3000 m`). `as` accepts `int8`, `int16`, `int32`,
`int64` and the `uint` variants, and reinterprets the integer in two's complement:
`255 as int8` = `-1`.

### Surprises

These differ from C, Rust or Python. Use parentheses when you mix them.

**Bitwise and shift operators share the multiplicative level.** `&`, `|`, `^`, `<<` and `>>`
bind exactly as tightly as `*` and `/` and evaluate left to right with them. In C and Rust they
are five separate levels, all looser than `*`.

| Input | numbr | C / Rust |
|---|---|---|
| `1 \| 2 * 3` | `(1 \| 2) * 3` = `9` | `1 \| (2 * 3)` = `7` |
| `1 \| (2 * 3)` | `7` | `7` |
| `1 << 4 + 1` | `(1 << 4) + 1` = `17` | `1 << (4 + 1)` = `32` |

**Prefix minus binds tighter than `**`.** `-2 ** 2` is `(-2) ** 2` = `4`, not `-4` as in
Python. Write `-(2 ** 2)` for `-4`.

**A unit name binds tighter than prefix minus and attaches to the nearest operand.**
`parse_unary` calls `parse_postfix`, so `-1 celsius` parses as `-(1 celsius)`. Negating a unit
value negates its amount, so this gives the expected result: `-1 celsius in K` = `272.15 K`.
The unit only covers the operand right before it:

| Input | Parses as | Result |
|---|---|---|
| `2 * 3 km` | `2 * (3 km)` | `6 km` |
| `2 + 3 km` | `2 + (3 km)` | type error, `cannot add 2 and 3 km` |
| `(2 + 3) km` | `(2 + 3) km` | `5 km` |
| `2 ** 3 km` | `2 ** (3 km)` | type error |

**`%` is percent, not remainder.** It is postfix only, so `7 % 3` is a parse error. The
remainder operator is `mod`: `7 mod 3` = `1`.

**Any `NNNN-N-N` is a date.** The lexer reads four digits, `-`, one or two digits, `-`, one or
two digits as a date literal, so `1000-2-3` is the date `1000-02-03`, not `995`, and
`2026-1-1 + 1` is a type error (`cannot add 2026-01-01 and 1`). Put spaces around `-` for
subtraction.

**Percent.** Postfix `%` turns a number into a fraction: `10%` = `0.10`, `2 * 3 %` =
`2 * 0.03` = `0.06`, `100 * 10%` = `10.00`. Three rules sit on top of that:

- `X + N%` and `X - N%` mean "increase / decrease X by N percent". The interpreter
  (`interpreter.rs`, `Expr::BinaryOp`) checks whether the right operand is a bare percent before
  it evaluates anything: `100 + 10%` = `110.00`, `100 - 10%` = `90.00`. Any other operator
  uses the plain fraction.
- `X% of Y` is only parsed when the whole left side is a bare percent. `5% of 200` = `10.00`, but
  `5 of 200`, `-5% of 200` and `50 + 10% of 200` are parse errors. Write
  `50 + (10% of 200)` = `70.00`.
- The right side of `of` is a full additive expression: `10% of 50 + 5` is `10% of (50 + 5)` =
  `5.50`.

## Tokens

Defined in `crates/core/src/lexer.rs`. Spaces, tabs and `\r` are skipped. Where two token forms
could match, the longest match wins, so `2026-07-04` is one date, not `2026 - 7 - 4`.

| Form | Examples | Notes |
|---|---|---|
| Decimal integer | `42`, `1_000_000` | `_` separators anywhere after the first digit. Stored as `i128`; a literal that does not fit is a parse error. |
| Hex, binary, octal | `0xFF`, `0b1010`, `0o17`, `0xFF_FF`, `0X1f` | Prefix letter in either case, `_` after the first digit. |
| Float | `1.5`, `1_000.5`, `1.5e3`, `2e3`, `1.5E-2` | Starts with a digit; a dot needs a digit on both sides (`.5` and `1.` are errors). |
| Decimal comma | `1,5` | Only with the comma separator setting, then `1,5` = `1.5`. With the default point setting `1,5` lexes as `1`, `,`, `5`. |
| String | `"abc"` | Double quotes. A backslash keeps the next character inside the string but is not an escape: `"a\"b"` evaluates to `a\"b`. `+` concatenates: `"a" + "b"` = `ab`. |
| Date | `2026-07-04`, `2026-7-4` | `YYYY-M-D` with one or two digit month and day, so `1000-2-3` is a date too (see Surprises). An invalid date such as `2026-02-30` is a parse error. Spaced out, `2026 - 07 - 04` is subtraction (`2015`). |
| Identifier | `price`, `line3`, `km`, `_tmp` | `[a-zA-Z_][a-zA-Z0-9_]*`. |
| Keyword | `in`, `as`, `of`, `mod` | Reserved, never identifiers. `mod` is the remainder operator, not a function: `7 mod 4` = `3`. |
| Namespace | `modbus::float32` | `::` between identifiers, see Function calls. |
| Operators | `+ - * / % ** & \| ^ ~ << >>` | `%` is postfix percent only, see Surprises. |
| Punctuation | `= ( ) , ;` | |

`#` starts a comment that runs to the end of the line. It is not a token: `strip_comment` in
`crates/core/src/engine.rs` removes it before the lexer runs. A `#` inside a string literal is
kept: `"a#b"` = `a#b`, `1 + 2 # note` = `3`.

### Identifiers

An identifier resolves in this order (`interpreter.rs`, `Expr::Ident`):

1. Constants: `pi`/`PI`, `e`/`E`, `phi`, `today`, `now`, and the Modbus word orders `ABCD`,
   `CDAB`, `BADC`, `DCBA`, which evaluate to themselves as strings. Constants cannot be
   shadowed: after `e = 5`, `e` is still `2.718281828`.
2. `lineN`, the result of line N.
3. Variables.

Anything else is an unknown variable. An identifier directly after an operand is read as a
unit when `units::is_known_unit` knows the name (`2 km`); otherwise it is a parse error
(`2 foo`). `in` is a keyword, so the inch unit is spelled `inch` or `inches`.

### Function calls

`name(arg, arg, ...)`. The parentheses are required (`sqrt 16` is a parse error) and each
argument is a full expression, `;` included. With the comma separator setting, `f(1,5)` is one
argument `1.5`; write `f(1, 5)` for two.

Namespaced names are joined with `_` in the parser, so `modbus::float32(0x4128, 0x0000)` and
`modbus_float32(0x4128, 0x0000)` are the same call (`10.5`). A `FunctionProvider` sees the
underscore form in `provides` and `call`.

## Evaluation model

The UI evaluates a document one line at a time with `Engine::evaluate_line`
(`crates/core/src/engine.rs`):

- **One expression per line.** Use `;` to put several on one line; all are evaluated and the
  last one is the line's result.
- **Line references.** Every line records its result, so `lineN` (1-based) refers to it.
  Blank lines and comment-only lines count; their result is empty.
- **Variables.** `name = expr` stores the value, and the variable is visible on every later
  line. The value is computed once: after `x = 5`, `y = x * 2`, `x = 1`, `y` is still `10`.
- **Errors.** A parse error, incomplete input or an unknown variable gives an empty result,
  so a half-typed line shows nothing. Other errors show as `Error: ...`, for example
  `1 / 0` shows `Error: Division by zero`.
- **Integers.** Integer `+`, `-` and `*` wrap around at the `i128` limits. `/` gives a float
  when the division is not exact: `7 / 2` = `3.5`. Known bug: prefix `-`, `/ -1` and
  `mod -1` on the smallest `i128` value are not handled and panic the evaluator (a release
  build wraps the prefix `-` instead).

### `result:` aggregates

A line of the form `result: <aggregate>` aggregates the numbers above it
(`Engine::evaluate_result_command`):

| Aggregate | Value |
|---|---|
| `sum` | Sum |
| `average`, `avg`, `mean` | Arithmetic mean |
| `median` | Middle value, mean of the two middle values for an even count |
| `min`, `max` | Smallest, largest |
| `count` | Number of values |

The name is case-insensitive (`result: MEAN`). An unknown name, and any prefix of
`result` typed so far, gives an empty result. Known quirk: that prefix check runs before
parsing, so a line that is exactly `r`, `re`, `res`, `resu`, `resul` or `result` shows an
empty result even when a variable of that name exists. `r + 0` shows the value.

The aggregation window is the contiguous block directly above the `result:` line
(`contiguous_result_numbers`):

- It ends at the nearest line with an empty result going up: a blank line, a comment-only
  line, or a line with a parse error or unknown variable. Empty lines directly above the
  `result:` line are skipped first.
- Inside the block, values that are not plain numbers are skipped without ending it: unit
  values (`2 km`), strings, `in hex` results, and errors.
- A `result:` line's own value is part of the block for the lines below it. Two
  `result: sum` lines in a row give the sum, then twice the sum. Under `1`, `5`, `3`,
  `result: min` (`1`) and `result: max` (`5`), `result: avg` is `3`, the mean of
  `1 5 3 1 5`.
- An empty window is a type error, `result aggregate has no numeric values`.

```
10
20

1
2
3
result: sum      6
result: sum      12
```

## Grammar sketch

EBNF for reference; the tables above are normative where they say more.

```ebnf
sequence       = assign { ";" [ assign ] } ;
assign         = IDENT "=" convert | convert ;
convert        = percent_of { ( "in" IDENT ) | ( "as" IDENT ) } ;
percent_of     = additive [ "of" additive ] ;      (* only if the left side is a bare percent *)
additive       = multiplicative { ( "+" | "-" ) multiplicative } ;
multiplicative = power { ( "*" | "/" | "mod" | "&" | "|" | "^" | "<<" | ">>" ) power } ;
power          = unary [ "**" power ] ;
unary          = ( "-" | "~" ) unary | postfix ;
postfix        = primary [ "%" | UNIT ] ;
primary        = INT | HEX | BIN | OCT | FLOAT | STRING | DATE
               | IDENT { "::" IDENT } [ "(" [ sequence { "," sequence } ] ")" ]
               | "(" sequence ")" ;
```
