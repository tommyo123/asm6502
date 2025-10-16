//! 6502 minimal assembler with optional human-readable listing (feature: "listing")
//! - Strict hex-only syntax ($ for hex numbers)
//! - Optional address-mode forcing with operand prefixes:
//!     "<" => force Zero Page (e.g. LDA <$80, LDA <$80,X)
//!     ">" => force Absolute  (e.g. LDA >$80, LDA >$80,X)
//! - Adaptive long-branch fixing pass count (bounded by number of branches + 2)
//!
//! ## Features
//! - **Hex-only syntax** (`$` prefix for hex numbers).
//! - **Directives**:
//!   - `*=$xxxx` — set program origin (ORG).
//!   - `DCB $nn ...` — define raw bytes.
//! - **Addressing modes** supported (immediate, zeropage, absolute, indexed, indirect).
//! - **Branch fixing**: automatically rewrites long branches into short branch + `JMP`.
//! - **Force addressing mode** using operand prefixes:
//!   - `<` → force Zero Page (e.g. `LDA <$80`).
//!   - `>` → force Absolute (e.g. `LDA >$80`).
//!
//! ## Optional Features
//! - `listing`: enables functions to print and save human-readable assembly listings.
//!
//! ## Basic Usage
//! ```rust
//! use asm6502::Assembler6502;
//!
//! fn main() -> Result<(), asm6502::AsmError> {
//!     let mut assembler = Assembler6502::new();
//!     let src = r#"
//!         *=$0800
//!         LDA #$42
//!         STA $0200
//!     "#;
//!
//!     let bytes = assembler.assemble_bytes(src)?;
//!     assert_eq!(bytes, vec![0xA9, 0x42, 0x8D, 0x00, 0x02]);
//!     Ok(())
//! }
//! ```
//!
//! ## License
//! This project is released under [The Unlicense](https://unlicense.org/).
//! You are free to use it for any purpose, without restriction.

mod error;
mod opcodes;
mod symbol;
mod parser;
mod addressing;
mod eval;
mod assembler;

// Public exports
pub use error::AsmError;
pub use assembler::{Assembler6502, Item};
