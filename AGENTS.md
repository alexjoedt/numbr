# AGENTS.md

## Project Overview

**numbr** is a natural-language calculator for Linux/Wayland, optimized for Hyprland. It is written in Rust and structured as a Cargo workspace.

## Workspace Layout

```
crates/
  core/       # Evaluation engine: lexer, parser, interpreter, builtins, value types
  ui/         # Iced-based GUI (multi-window, Wayland)
  providers/  # External data providers (e.g. exchange rates)
  app/        # Binary entry point
```

## Build & Run

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run
cargo run
```

## Testing

```bash
# Run all tests
cargo test

# Run tests for a specific crate
cargo test -p numbr-core

# Update insta snapshots
cargo insta review
```

## Key Dependencies

| Crate | Purpose |
|---|---|
| `logos` | Lexing / tokenisation |
| `rust_decimal` | Arbitrary-precision decimal arithmetic |
| `chrono` | Date/time support |
| `iced` | GUI framework (Wayland / multi-window) |
| `thiserror` | Error type derivation |
| `insta` | Snapshot testing |

## Coding Guidelines

- Follow standard Rust idioms and the Rust API Guidelines.
- Use `rust_decimal::Decimal` for all numeric values — avoid `f32`/`f64` for calculator logic.
- Errors in `core` use `thiserror`-derived types (`EvalError`, `FuncError`).
- New language features (functions, builtins, operators) belong in `crates/core`.
- UI concerns belong in `crates/ui`; keep `core` free of GUI dependencies.
- Write snapshot tests with `insta` for any new parser or interpreter behaviour.
- Keep `crates/app` minimal — it should only wire up `core`, `ui`, and `providers`.
