# Changelog

All notable changes to numbr are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Pull requests with a user-visible change add a line under `[Unreleased]`, in the
matching `Added`, `Changed`, `Fixed` or `Removed` group. Describe the behaviour a user
sees, not the commit. On release, the `[Unreleased]` entries move to a new version
section and the compare links at the bottom are updated.

## [Unreleased]

## [0.2.0] - 2026-10-09

### Changed

- `numbr-core` exposes only the engine, value and provider types. The lexer, parser,
  interpreter, builtin, modbus and unit modules are now crate-private.
- The 0.1.2 release binaries report version 0.1.1, because the workspace version was
  not bumped before tagging. 0.2.0 is the first release whose version matches its tag.

### Fixed

- Huge numbers no longer crash the app. A unit amount or percentage beyond about 7.9e28,
  a unit or percentage result out of range and an overflowing `result:` aggregate show
  an error. `-`, `/ -1`, `mod -1` and `abs` on the smallest integer wrap like `+` and `*`.
  Such a huge integer mixed with a percentage or decimal result is an error instead of a
  float, and in a `result:` block it is an error instead of being skipped. `result: avg`
  of values near the limit no longer fails when their sum would overflow.
- Deeply nested or very long expressions no longer crash the app. More than 64 nesting
  levels or 256 operators on one line is a parse error.

## [0.1.2] - 2026-09-22

### Added

- Decimal separator setting: choose point or comma in the settings panel. Results are
  formatted with the chosen separator, and number literals accept it. With comma
  selected, a comma between digits is part of the number: `modbus::int32(0,1)` reads
  `0,1` as one argument, so such lines in a restored session change meaning when you
  switch. Separate arguments with `, ` (comma and space).
- Scientific notation without a decimal point, such as `1e9`.
- Dual license: MIT or Apache-2.0.
- Declared minimum supported Rust version: 1.90.0 for building from source.

### Fixed

- Black window on hybrid GPU systems under Hyprland. numbr now renders on the
  high-performance GPU.
- Integer division no longer truncates: `22 / 5` returns `4.4`.
- Negative quantities with a unit, such as `-5 km` or `-1 celsius`, no longer fail.
- Fractional durations convert, such as `0.1 in years`.
- Date arithmetic that leaves the supported range, such as `today + 99999999 years`,
  reports an error instead of crashing.
- An invalid background color in `settings.toml` falls back to the default instead of
  crashing.

## [0.1.1] - 2026-07-05

### Added

- Background color and transparency settings in `settings.toml`.

## [0.1.0] - 2026-07-05

### Added

- Arithmetic with `+`, `-`, `*`, `/`, `**` and `mod`, and percentages (`5% of 200`,
  `price - 20%`).
- Variables (`price = 42`), line references (`line1`), comments (`#`) and the `pi`
  constant.
- Aggregates over the current block: `result: sum`, `average`, `median`, `min`, `max`
  and `count`. Blank and non-numeric lines are skipped.
- Scientific functions: `sqrt`, `sin`, `cos`, `tan`, `log10`, `ln`, `exp`, `abs`,
  `round`, `floor`, `ceil`.
- Number systems: `0xFF`, `0b1010`, `0o17` literals, converted with `in hex`,
  `in binary` and `in octal`.
- Bit widths with two's complement: `255 as int8` gives `-1`.
- Bit operations: `&`, `|`, `^`, `~`, `<<`, `>>`, `popcount`, `clz`, `ctz`,
  `byteswap16`, `byteswap32`.
- Modbus register conversion: `modbus::float32`, `modbus::float32le`, `modbus::int32`,
  `modbus::uint32`, word-order variants, `modbus::swap::word` and `modbus::swap::byte`.
- Unit conversion for length, mass, temperature, pressure, data size (SI and IEC) and
  time.
- Date arithmetic: date literals, `today + 2 weeks`, `diff(2026-07-04, 2026-01-01)`.
- Session persistence: the last session is restored on startup.
- Clipboard: click a result to copy it.
- Font size, family and weight settings, stored in `settings.toml`.
- Hyprland integration instructions in the README.

[Unreleased]: https://github.com/alexjoedt/numbr/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/alexjoedt/numbr/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/alexjoedt/numbr/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/alexjoedt/numbr/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/alexjoedt/numbr/releases/tag/v0.1.0
