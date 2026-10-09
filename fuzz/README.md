# Fuzzing numbr-core

Two [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) targets drive the evaluator with
arbitrary UTF-8:

- `evaluate`: one `Engine::new().evaluate(input)` per input.
- `evaluate_lines`: splits the input into lines and feeds them to `Engine::evaluate_line` on
  one engine, so variables, `lineN` references and `result:` aggregates are exercised.

Inputs are not capped: the parser rejects nesting deeper than 64 levels and more than 256
operators per expression, so a stack overflow is a finding.

## Run

libFuzzer needs a nightly toolchain and a C++ compiler. `fuzz/` is its own workspace, so
stable `cargo build` and `cargo test` never touch it.

```bash
rustup toolchain install nightly
cargo install cargo-fuzz
cd fuzz
mkdir -p corpus/evaluate corpus/evaluate_lines
cargo +nightly fuzz run evaluate corpus/evaluate seeds/evaluate -- -max_total_time=1800
cargo +nightly fuzz run evaluate_lines corpus/evaluate_lines seeds/evaluate_lines -- -max_total_time=1800
```

`seeds/` is the committed seed corpus (README examples and the core test inputs). New
inputs go to `corpus/`, crashes to `artifacts/`; both are gitignored. Reproduce a crash
with `cargo +nightly fuzz run <target> artifacts/<target>/crash-<hash>` and turn it into a
regression test in `crates/core/src/lib.rs`.

Run it before a release. It is deliberately not part of CI.
