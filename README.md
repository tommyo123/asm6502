# Assembler6502

A minimal 6502 assembler written in Rust. It generates raw machine code from 6502 assembly source and, optionally, prints a human‑readable assembly listing when the `listing` feature is enabled.

## Features

* Hex‑only syntax (`$` prefix for numbers)
* Zero Page vs Absolute auto‑detection, with explicit overrides using operand prefixes:

  * `<$80` → force Zero Page addressing
  * `>$80` → force Absolute addressing
* Adaptive long‑branch expansion (out‑of‑range branches become `BRANCH skip` + `JMP target`)
* Optional listing output (print to stdout and/or save to file)
* Symbol table & address mapping helpers

## Workspace Layout

This repository is a Cargo **workspace** with a library crate and a runnable example crate:

```
asm6502/                         # ← workspace root (this dir)
├─ Cargo.toml                    # [workspace] only
├─ asm6502/                      # library crate (the assembler)
│  ├─ Cargo.toml
│  └─ src/
│     └─ Assembler6502.rs        # library source (or use src/lib.rs if you prefer)
└─ example/                      # example binary crate (demo / tests)
   ├─ Cargo.toml
   └─ src/
      └─ main.rs
```

### Library crate `asm6502`

* Exposes the public API (`Assembler6502`, `AsmError`, etc.)
* Listing helpers are gated behind the Cargo feature `listing`.
* If you keep the file name `Assembler6502.rs`, ensure your `asm6502/Cargo.toml` contains:

```toml
[lib]
name = "asm6502"
path = "src/Assembler6502.rs"
```

## Using the library from another project

Add a Git dependency in your project's `Cargo.toml`:

```toml
[dependencies]
asm6502 = { git = "https://github.com/tommyo123/asm6502" }
```

Optionally enable the listing feature:

```toml
asm6502 = { git = "https://github.com/tommyo123/asm6502", features = ["listing"] }
```

Minimal example:

```rust
use asm6502::Assembler6502;

fn main() {
    let mut asm = Assembler6502::new();
    let code = r#"
        *=$0800
        start:
            LDA #$42
            STA $0200
            RTS
    "#;

    let machine = asm.assemble_bytes(code).unwrap();
    println!("Assembled {} bytes", machine.len());
}
```

## Example crate (this repository)

The workspace includes a runnable `example` crate demonstrating the API.

### Run the example

From the workspace root:

```bash
# default run (no listing)
cargo run -p asm6502-example

# run with listing enabled in the example (Option A: feature forwarding)
cargo run -p asm6502-example --features listing
```

### Option A: feature forwarding (recommended)

`example/Cargo.toml` forwards its own `listing` feature to the library's feature:

```toml
[package]
name = "asm6502-example"
version = "1.0.0"
edition = "2021"

[dependencies]
asm6502 = { path = "../asm6502" }

[features]
default = []
listing = ["asm6502/listing"]   # forward the feature
```

In `example/src/main.rs` you can use:

```rust
#[cfg(feature = "listing")]
{ /* print or save listing */ }
```

### Option B: depend directly on the lib feature (no example-local features)

In `example/Cargo.toml`:

```toml
[dependencies]
asm6502 = { path = "../asm6502", features = ["listing"] }
```

And in `example/src/main.rs` gate on the dependency feature:

```rust
#[cfg(feature = "asm6502/listing")]
{ /* print or save listing */ }
```

## Building & Docs

Build the workspace:

```bash
cargo build
```

Build only the library:

```bash
cargo build -p asm6502
```

Generate local API docs:

```bash
cargo doc --open
```

## License

This project is released under [The Unlicense](LICENSE).
It is free and unencumbered software released into the public domain.
