# Contributing

Thanks for your interest. Start with [AGENTS.md](AGENTS.md) — it documents the workspace
layout, build and test commands, and the coding conventions this project follows.

## Before you start

numbr is a Wayland application and **builds on Linux only**. `crates/core` and
`crates/providers` build anywhere; `crates/ui` and `crates/app` do not. You need
`libwayland-dev libxkbcommon-dev libegl-dev` (see the README's
[Platform](README.md#platform) section for other distributions).

## What CI will run against your PR

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace
cargo test --workspace
```

Run them locally first.

## What a pull request should include

- Tests for any behaviour change. Engine tests live in the `mod tests` block in
  `crates/core/src/lib.rs`.
- `cargo fmt` applied and `cargo clippy` clean.
- One logical change per PR.

## Licensing

Contributions are dual licensed under MIT OR Apache-2.0, matching the project. By
submitting a contribution, you agree it is licensed under the same terms, without any
additional terms or conditions (inbound = outbound).
