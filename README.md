# numbr

A natural-language calculator for Linux/Wayland, optimized for Hyprland.
Inspired by [Numi](https://numi.app), with a focus on developer workflows.

```
10 km in miles                   6.21371 miles
sqrt(2) * 3                      4.242640687
0xFF in binary                   0b11111111
255 as int16                     -1
price = 42; price * 3            126
modbus::float32(0x4128, 0x0000)  10.5
today + 2 weeks                  2026-07-18
```

## Features

- Arithmetic, percentages (`5% of 200`, `price - 20%`), variables, line references (`line1`)
- Scientific functions: `sqrt`, `sin`, `cos`, `tan`, `log`, `ln`, `exp`, `abs`, …
- Number systems: `0xFF`, `0b1010`, `0o17` — convert with `in hex`, `in binary`, `in octal`
- Bit widths: `255 as int8` → `-1` (two's complement), `0xFFFF as int16` → `-1`
- Bit operations: `&`, `|`, `^`, `~`, `<<`, `>>`, `popcount`, `clz`, `ctz`
- Modbus register conversion: `modbus::float32`, `modbus::int32`, `modbus::uint32`, word-order variants
- Unit conversion: length, mass, temperature, pressure, data size, time
- Date arithmetic: `today + 2 weeks`, `diff(2026-07-04, 2026-01-01) in days`
- Session persistence: last session is automatically restored on startup
- Clipboard: click any result to copy it

## Build

```bash
cargo build --release
```

The binary is placed at `target/release/numbr`.

## Hyprland integration

Add to your `hyprland.conf`:

```conf
# Launch numbr with Super+Shift+C
bind = SUPER_SHIFT, C, exec, numbr

# Float it, centre it, give it a fixed size
windowrulev2 = float,        class:^(numbr)$
windowrulev2 = size 680 520, class:^(numbr)$
windowrulev2 = center,       class:^(numbr)$
```

Close the window when you are done — your session is saved automatically and
restored next time you open numbr.

## Usage tips

- **Multi-line notebook** — each line is evaluated independently; previous results
  are accessible as `line1`, `line2`, …
- **Variables** — `price = 42; price * 3`
- **Comments** — lines starting with `#` are ignored
- **Clipboard** — click any result in the right column to copy it
