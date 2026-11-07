# AndroidFiles

A desktop file manager for Android devices over adb

## Requirements

- `adb` on `PATH`, or its path set in Settings
- Rust toolchain

## Run

```sh
cargo run
```

## Build

```sh
cargo build --release
./target/release/AndroidFiles
```

For a smaller, slightly slower binary, add `strip = true` to
`[profile.release]` (or pass `RUSTFLAGS="-C strip=symbols"` for a one-off
build).

## Test

```sh
cargo test
```

Contributor and architecture notes: [AGENTS.md](AGENTS.md).
