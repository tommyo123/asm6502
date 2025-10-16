# Assembler6502

A minimal 6502 assembler written in Rust for inline assembly and lightweight compilation. It generates raw machine code from 6502 assembly source and optionally prints a human-readable assembly listing when the `listing` feature is enabled.

## Features

* **Multiple number formats:**
    * Hexadecimal: `$FF`, `0xFF`, `0xFFh`
    * Binary: `%11111111`, `0b11111111`
    * Decimal: `255`
* **Expression arithmetic:**
    * Addition: `$10+5`, `LABEL+1`
    * Subtraction: `$FF-10`, `*-2`
    * Multiplication: `10*2`
    * Division: `100/5`
    * Mixed formats: `$10+10+%00000101`
    * Operator precedence: `*,/` before `+,-`
* **Constants:** Define reusable values with `LABEL = value` syntax
    * Simple: `SCREEN = $0400`
    * Expressions: `OFFSET = BASE+$10`
    * Current address: `HERE = *`, `NEXT = *+1`
* **Modern directives:**
    * `.byte` - Comma-separated bytes
    * `.word` - 16-bit words (little-endian)
    * `.string` - ASCII text strings
    * `.incbin` - Include binary files
* **Label arithmetic:** Use labels in expressions (`LDA buffer+1`, `JMP start+3`)
* **Current address symbol:** `*` represents the current program counter
* **Addressing mode control:**
    * Auto-detection of Zero Page vs Absolute
    * Explicit override with operand prefixes:
        * `<$80` → force Zero Page addressing
        * `>$80` → force Absolute addressing
* **Adaptive long-branch expansion:** Out-of-range branches automatically become `BRANCH skip` + `JMP target`
* **Optional listing output:** Print to stdout and/or save to file (feature-gated)
* **Symbol table & address mapping helpers**

## Syntax Guide

### Number Formats
```asm
LDA #$FF            ; Hexadecimal
LDA #255            ; Decimal
LDA #%11111111      ; Binary
LDA #0xFF           ; Alternative hex format
LDA #0b11111111     ; Alternative binary format
```

### Expressions
```asm
; Simple arithmetic
LDA #$02+1          ; = $03
LDA #10*2           ; = 20
LDA #100/5          ; = 20
LDA #$FF-10         ; = $F5

; Complex expressions
LDA #10*2+5         ; = 25 (precedence: * before +)
LDA #$10+10+%00000101  ; Mixed formats = $1F

; With labels
LDA buffer+1        ; Address of buffer + 1
JMP start+3         ; Jump to start + 3 bytes
```

### Constants
```asm
; Simple constants
SCREEN = $0400
SPRITE_X = 100
MAX_LIVES = 3

; Expression constants
BASE = $1000
OFFSET = BASE+$10   ; = $1010
DOUBLE = SPRITE_X*2 ; = 200

; Current address (*) constants
HERE = *            ; Current program counter
NEXT = *+1          ; Current PC + 1
SKIP = *+3          ; Skip next instruction

; Usage examples
    LDA #SPRITE_X
    STA SCREEN
    JMP HERE        ; Infinite loop

; Practical use of *+offset
skip_target:
RETURN_ADDR = *+1   ; Address of next instruction
    JSR subroutine  ; JSR is 3 bytes
    NOP             ; This is at RETURN_ADDR
    
; Conditional skip pattern
    LDA flag
    BEQ skip_load   ; If zero, skip the load
    LDA #$42        ; This gets skipped if flag=0
skip_load:
    STA result
```

### Directives

#### Origin and Data
```asm
*=$0800             ; Set origin (ORG)
DCB $01 $02 $03     ; Define bytes (space-separated, legacy)
```

#### Modern Data Directives
```asm
; .byte - Comma-separated bytes
.byte $FF,$FE,$FD
.byte $01,$02,$03,$04

; .word - 16-bit words (little-endian)
.word $1234         ; Assembles to: $34 $12
.word $1234,$5678   ; Assembles to: $34 $12 $78 $56

; .string - ASCII text
.string "HELLO"     ; Assembles to: $48 $45 $4C $4C $4F
.string "6502 ASM"

; .incbin - Include binary file
.incbin "data.bin"  ; Includes entire file as bytes
.incbin "sprite.dat"
```

#### Data Directive Comparison
```asm
; Old style (still supported)
DCB $01 $02 $03     ; Space-separated

; New style (recommended)
.byte $01,$02,$03   ; Comma-separated
```

### Labels
```asm
start:              ; Define label
    LDA #$42
    JMP start       ; Forward/backward references work

buffer:
    DCB $00 $00
    LDA buffer+1    ; Label arithmetic
```

### Complete Example
```asm
*=$0800

; Constants
SCREEN = $0400
CHAR_CODE = 65      ; 'A'

; Data section
message:
    .string "HELLO WORLD"
    
sprite_data:
    .byte $00,$3C,$42,$42,$7E,$42,$42,$00
    
lookup_table:
    .word $1000,$2000,$3000,$4000

; Code section
start:
    LDX #0
loop:
    LDA message,X
    STA SCREEN,X
    INX
    CPX #11
    BNE loop
    RTS
```

## Workspace Layout

This repository is a Cargo **workspace** with a library crate and a runnable example crate:

```
asm6502/                         # workspace root
├─ Cargo.toml                    # [workspace] only
├─ asm6502/                      # library crate (the assembler)
│  ├─ Cargo.toml
│  └─ src/
│     ├─ lib.rs
│     ├─ assembler.rs
│     ├─ opcodes.rs
│     ├─ symbol.rs
│     ├─ error.rs
│     ├─ addressing.rs
│     ├─ parser/
│     │  ├─ mod.rs
│     │  ├─ lexer.rs
│     │  ├─ expression.rs
│     │  └─ number.rs
│     └─ eval/
│        ├─ mod.rs
│        └─ expression.rs
└─ example/                      # example binary crate (tests/demo)
   ├─ Cargo.toml
   └─ src/
      └─ main.rs
```

### Library crate `asm6502`

* Exposes the public API (`Assembler6502`, `AsmError`, etc.)
* Listing helpers are gated behind the Cargo feature `listing`

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

### Basic Example

```rust
use asm6502::Assembler6502;

fn main() -> Result<(), asm6502::AsmError> {
    let mut asm = Assembler6502::new();
    
    let code = r#"
        *=$0800
        SCREEN = $0400
        
        start:
            LDA #$42
            STA SCREEN
            JMP start
    "#;

    let bytes = asm.assemble_bytes(code)?;
    println!("Assembled {} bytes", bytes.len());
    
    Ok(())
}
```

### Using New Directives

```rust
use asm6502::Assembler6502;

fn main() -> Result<(), asm6502::AsmError> {
    let mut asm = Assembler6502::new();
    
    let code = r#"
        *=$0800
        
        ; Define data using modern directives
        message:
            .string "HELLO"
            
        values:
            .byte $FF,$FE,$FD,$FC
            
        addresses:
            .word $1000,$2000,$3000
            
        ; Use the data
        start:
            LDX #0
        loop:
            LDA message,X
            STA $0400,X
            INX
            CPX #5
            BNE loop
            RTS
    "#;

    let bytes = asm.assemble_bytes(code)?;
    println!("Assembled {} bytes", bytes.len());
    
    Ok(())
}
```

### Advanced Example with Expressions

```rust
use asm6502::Assembler6502;

fn main() -> Result<(), asm6502::AsmError> {
    let mut asm = Assembler6502::new();
    asm.set_origin(0x1000);
    
    let code = r#"
        ; Constants
        BASE = $2000
        OFFSET = BASE+$100
        COUNT = 10*2
        
        ; Code with expressions
        start:
            LDA #COUNT
            STA BASE
            LDA OFFSET+5
            JMP start
    "#;

    let (bytes, symbols) = asm.assemble_with_symbols(code)?;
    
    println!("Assembled {} bytes", bytes.len());
    println!("\nSymbols:");
    for (name, addr) in symbols.iter() {
        println!("  {} = ${:04X}", name, addr);
    }
    
    Ok(())
}
```

### With Listing (feature-gated)

```rust
use asm6502::Assembler6502;

fn main() -> Result<(), asm6502::AsmError> {
    let mut asm = Assembler6502::new();
    
    let code = r#"
        *=$0800
        
        data:
            .byte $01,$02,$03
            .word $1234
            .string "HI"
            
        start:
            LDA #$42
            STA $0200
            RTS
    "#;

    #[cfg(feature = "listing")]
    {
        let (bytes, items) = asm.assemble_full(code)?;
        asm.print_assembly_listing(&items);
        asm.save_listing(&items, "output.lst")?;
    }
    
    #[cfg(not(feature = "listing"))]
    {
        let bytes = asm.assemble_bytes(code)?;
    }
    
    Ok(())
}
```

## Example crate (this repository)

The workspace includes a comprehensive test suite demonstrating all features:

### Run the example

From the workspace root:

```bash
# Run test suite
cargo run -p asm6502-example

# Run with listing enabled
cargo run -p asm6502-example --features listing
```

The test suite covers:
1. Number formats (hex, decimal, binary)
2. Expression arithmetic
3. Label arithmetic
4. Mixed number formats
5. Constants (`LABEL = value`)
6. Current address usage (`*`)
7. New directives (`.byte`, `.word`, `.string`)
8. Complete 6502 program (all addressing modes)

## API Overview

### Core Methods

```rust
// Simple assembly
fn assemble_bytes(&mut self, src: &str) -> Result<Vec<u8>, AsmError>

// Assembly with symbol table
fn assemble_with_symbols(&mut self, src: &str) 
    -> Result<(Vec<u8>, HashMap<String, u16>), AsmError>

// Full assembly with items (for listing)
fn assemble_full(&mut self, src: &str) 
    -> Result<(Vec<u8>, Vec<Item>), AsmError>

// Assembly with address mapping
fn assemble_with_addr_map(&mut self, src: &str)
    -> Result<(Vec<u8>, Vec<(usize, u16)>), AsmError>

// Configuration
fn set_origin(&mut self, addr: u16)
fn origin(&self) -> u16
fn reset(&mut self)

// Symbol inspection
fn symbols(&self) -> &HashMap<String, u16>
fn lookup(&self, name: &str) -> Option<u16>

// Binary output
fn write_bin<W: Write>(bytes: &[u8], w: W) -> io::Result<()>
```

### Listing Methods (feature-gated)

```rust
#[cfg(feature = "listing")]
fn print_assembly_listing(&self, items: &[Item])

#[cfg(feature = "listing")]
fn save_listing(&self, items: &[Item], filename: &str) -> io::Result<()>
```

## Directive Reference

| Directive | Syntax | Description | Example |
|-----------|--------|-------------|---------|
| `*=` | `*=$0800` | Set origin address | `*=$C000` |
| `DCB` | `DCB $01 $02` | Define bytes (space-separated) | `DCB $FF $00` |
| `.byte` | `.byte $01,$02` | Define bytes (comma-separated) | `.byte $01,$02,$03` |
| `.word` | `.word $1234` | Define 16-bit words (little-endian) | `.word $1000,$2000` |
| `.string` | `.string "text"` | Define ASCII string | `.string "HELLO"` |
| `.incbin` | `.incbin "file"` | Include binary file | `.incbin "data.bin"` |
| `LABEL =` | `CONST = $42` | Define constant | `SCREEN = $0400` |

## Building & Docs

Build the workspace:

```bash
cargo build
```

Build only the library:

```bash
cargo build -p asm6502
```

Run tests:

```bash
cargo test
```

Generate local API docs:

```bash
cargo doc --open
```

## Design Philosophy

This assembler is designed for:
- **Inline assembly**: Quick compilation of small code snippets
- **JIT compilation**: Runtime generation of 6502 code
- **Emulator testing**: Dynamic test case generation
- **Educational tools**: Interactive 6502 learning
- **Simplicity**: Minimal dependencies, clear code structure

It intentionally **does not** include:
- Multi-file projects or linking
- Macro systems
- Complex multi-pass constant resolution
- Object file formats

Constants and expressions are evaluated in-order, requiring definitions before use (except labels which support forward references).

## Technical Notes

- **Forward references:** Labels support forward references, constants do not
- **Best practice:** Define constants at the top of your source
- **Expression evaluation:** Left-to-right with standard precedence (`*`, `/` before `+`, `-`)
- **Branch range:** Automatic long-branch expansion for out-of-range branches
- **Word endianness:** `.word` directive outputs little-endian (6502 native format)
- **String encoding:** `.string` uses standard ASCII encoding
- **Binary inclusion:** `.incbin` reads files relative to working directory
- **Current address arithmetic:**
    - `LABEL = *` captures the current program counter
    - `LABEL = *+n` useful for calculating addresses of upcoming instructions
    - Example: `RETURN_ADDR = *+1` before a `JSR` captures the return address
    - Can be used for self-modifying code or address table generation

## Version History

### v2.0 (Current)
- Added `.byte`, `.word`, `.string`, and `.incbin` directives
- Improved listing output for data directives
- Enhanced address mapping support

### v1.0
- Initial release with core 6502 assembly support
- Expression evaluation and constants
- Adaptive long-branch expansion
- Optional listing feature

## License

This project is released under [The Unlicense](LICENSE).
It is free and unencumbered software released into the public domain.