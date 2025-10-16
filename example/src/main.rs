use std::fs::File;
use asm6502::{Assembler6502, AsmError};

fn main() -> Result<(), AsmError> {
    println!("=== 6502 Assembler Test Suite ===\n");

    // Test 1: Number formats
    test_number_formats()?;

    // Test 2: Expression arithmetic
    test_expressions()?;

    // Test 3: Label arithmetic
    test_label_arithmetic()?;

    // Test 4: Mixed number formats
    test_mixed_formats()?;

    // Test 5: Constants (LABEL = value)
    test_constants()?;

    // Test 6: Current address usage
    test_current_address()?;

    // Test 7: Complete demo program
    test_complete_program()?;

    println!("\n✅ All tests passed successfully!");

    Ok(())
}

fn test_number_formats() -> Result<(), AsmError> {
    println!("📝 Test 1: Number Formats");
    println!("{}", "=".repeat(50));

    let mut assembler = Assembler6502::new();

    let test_code = r#"
*=$0800
    ; Hexadecimal formats
    LDA #$FF            ; Standard hex
    LDA #$42

    ; Decimal format
    LDA #255            ; Decimal 255 = $FF
    LDA #66             ; Decimal 66 = $42

    ; Binary format
    LDA #%11111111      ; Binary = $FF
    LDA #%01000010      ; Binary = $42

    ; Mixed in DCB
    DCB $10 32 %00110011
"#;

    let bytes = assembler.assemble_bytes(test_code)?;

    println!("✓ Assembled {} bytes", bytes.len());
    println!("  - Hex format: $FF → ${:02X}", bytes[1]);
    println!("  - Decimal 255 → ${:02X}", bytes[5]);
    println!("  - Binary %11111111 → ${:02X}", bytes[9]);
    println!();

    Ok(())
}

fn test_expressions() -> Result<(), AsmError> {
    println!("📝 Test 2: Expression Arithmetic");
    println!("{}", "=".repeat(50));

    let mut assembler = Assembler6502::new();

    let test_code = r#"
*=$0800
    ; Addition
    LDA #$02+1          ; = $03
    LDA #10+5           ; = $0F (15)

    ; Subtraction
    LDA #$FF-1          ; = $FE
    LDA #100-50         ; = $32 (50)

    ; Multiplication
    LDA #10*2           ; = $14 (20)
    LDA #5*5            ; = $19 (25)

    ; Division
    LDA #100/5          ; = $14 (20)
    LDA #$FF/2          ; = $7F (127)

    ; Complex expressions
    LDA #10*2+5         ; = 25 ($19)
    LDA #100/5-4        ; = 16 ($10)
"#;

    let bytes = assembler.assemble_bytes(test_code)?;

    println!("✓ Assembled {} bytes", bytes.len());
    println!("  - $02+1 → ${:02X} (expected $03)", bytes[1]);
    println!("  - 10*2 → ${:02X} (expected $14)", bytes[9]);
    println!("  - 100/5 → ${:02X} (expected $14)", bytes[13]);
    println!("  - 10*2+5 → ${:02X} (expected $19)", bytes[17]);
    println!();

    Ok(())
}

fn test_label_arithmetic() -> Result<(), AsmError> {
    println!("📝 Test 3: Label Arithmetic");
    println!("{}", "=".repeat(50));

    let mut assembler = Assembler6502::new();

    let test_code = r#"
*=$0800
buffer:
    DCB $00 $00 $00 $00

start:
    ; Load from buffer+1
    LDA buffer+1        ; Address $0801

    ; Store to buffer+2
    STA buffer+2        ; Address $0802

    ; Indexed with offset
    LDA buffer+1,X      ; ZP mode if buffer < $100

    ; Jump to label+offset
    JMP start+3
"#;

    let (bytes, symbols) = assembler.assemble_with_symbols(test_code)?;

    println!("✓ Assembled {} bytes", bytes.len());
    println!("  Symbols:");
    for (name, addr) in symbols.iter() {
        println!("    {} = ${:04X}", name, addr);
    }
    println!("  - buffer+1 used in LDA → ${:04X}",
             u16::from_le_bytes([bytes[5], bytes[6]]));
    println!();

    Ok(())
}

fn test_mixed_formats() -> Result<(), AsmError> {
    println!("📝 Test 4: Mixed Number Formats");
    println!("{}", "=".repeat(50));

    let mut assembler = Assembler6502::new();

    let test_code = r#"
*=$0800
    ; Mix hex + decimal
    LDA #$42+1          ; Hex + decimal = $43
    LDA #255-$10        ; Decimal - hex = $EF (239)

    ; Mix binary + decimal
    LDA #%11111111-127  ; Binary - decimal = $80 (128)
    LDA #10+%00001111   ; Decimal + binary = $19 (25)

    ; Mix hex + binary
    LDA #$10+%00001111  ; Hex + binary = $1F (31)
    LDA #%11110000-$0F  ; Binary - hex = $E1 (225)

    ; All three mixed
    LDA #$10+10+%00000101  ; Hex + decimal + binary = $1F (31)
"#;

    let bytes = assembler.assemble_bytes(test_code)?;

    println!("✓ Assembled {} bytes", bytes.len());
    println!("  - $42+1 → ${:02X} (expected $43)", bytes[1]);
    println!("  - 255-$10 → ${:02X} (expected $EF)", bytes[3]);
    println!("  - %11111111-127 → ${:02X} (expected $80)", bytes[5]);
    println!("  - $10+%00001111 → ${:02X} (expected $1F)", bytes[9]);
    println!("  - $10+10+%00000101 → ${:02X} (expected $1F)", bytes[13]);
    println!();

    Ok(())
}

fn test_constants() -> Result<(), AsmError> {
    println!("📝 Test 5: Constants (LABEL = value)");
    println!("{}", "=".repeat(50));

    let mut assembler = Assembler6502::new();

    let test_code = r#"
*=$0800

; Define constants
SCREEN = $0400
COLOR = $D800
SPRITE_X = 100
SPRITE_Y = 200

; Use constants
    LDA #SPRITE_X
    STA SCREEN
    LDA #SPRITE_Y
    STA COLOR

; Constants with expressions
BASE = $1000
OFFSET = BASE+$10
DOUBLE = SPRITE_X*2

    LDA OFFSET          ; Should be $1010
    LDX #DOUBLE         ; Should be 200 ($C8)

; Current address constant
HERE = *
    JMP HERE
"#;

    let (bytes, symbols) = assembler.assemble_with_symbols(test_code)?;

    println!("✓ Assembled {} bytes", bytes.len());
    println!("  Constants defined:");
    for (name, value) in symbols.iter() {
        if !name.starts_with("__") && !name.ends_with(':') {
            println!("    {} = ${:04X} ({})", name, value, value);
        }
    }

    // Verify constants in assembled code
    assert_eq!(bytes[1], 100, "SPRITE_X constant");
    assert_eq!(bytes[6], 200, "SPRITE_Y constant");
    assert_eq!(bytes[14], 200, "DOUBLE constant");

    println!("✓ Constant verification passed");
    println!();

    Ok(())
}

fn test_current_address() -> Result<(), AsmError> {
    println!("📝 Test 6: Current Address (*) Usage");
    println!("{}", "=".repeat(50));

    let mut assembler = Assembler6502::new();

    let test_code = r#"
*=$0800

; Pattern using current address in origin
    NOP
    NOP

; Table generation using expressions
table:
    DCB $00             ; Will be at $0802
    DCB $01             ; Will be at $0803
    DCB $02             ; Will be at $0804

; Jump forward
here:
    NOP
    NOP
    NOP
    JMP table           ; Jump to table

end:
    RTS
"#;

    let (bytes, symbols) = assembler.assemble_with_symbols(test_code)?;

    println!("✓ Assembled {} bytes", bytes.len());
    println!("  Symbols:");
    for (name, addr) in symbols.iter() {
        if !name.starts_with("__") {
            println!("    {} = ${:04X}", name, addr);
        }
    }
    println!();

    Ok(())
}

fn test_complete_program() -> Result<(), AsmError> {
    println!("📝 Test 7: Complete Demo Program");
    println!("{}", "=".repeat(50));

    let mut assembler = Assembler6502::new();

    let test_code = r#"
*=$0080

    ; Zero page variables
    zp1:        DCB $20
    zp2:        DCB $30
    pointer:    DCB $40

; *** Load/Store with expressions ***
load_store:
    LDA #$42
    LDA zp1
    LDA zp1,X
    LDA $2000
    LDA $2000,X
    LDA $2000,Y
    LDA (pointer,X)
    LDA (pointer),Y

    LDX #$42
    LDX zp1
    LDX zp1,Y
    LDX $2000
    LDX $2000,Y

    LDY #$42
    LDY zp1
    LDY zp1,X
    LDY $2000
    LDY $2000,X

    STA zp1
    STA zp1,X
    STA $2000
    STA $2000,X
    STA $2000,Y
    STA (pointer,X)
    STA (pointer),Y

    STX zp1
    STX zp1,Y
    STX $2000

    STY zp1
    STY zp1,X
    STY $2000

; *** Arithmetic with expressions ***
arithmetic:
    ADC #$42
    ADC zp1
    ADC zp1,X
    ADC $2000
    ADC $2000,X
    ADC $2000,Y
    ADC (pointer,X)
    ADC (pointer),Y

    SBC #$42
    SBC zp1
    SBC zp1,X
    SBC $2000
    SBC $2000,X
    SBC $2000,Y
    SBC (pointer,X)
    SBC (pointer),Y

    INC zp1
    INC zp1,X
    INC $2000
    INC $2000,X

    DEC zp1
    DEC zp1,X
    DEC $2000
    DEC $2000,X

    INX
    INY
    DEX
    DEY

; *** Logical ***
logical:
    AND #$42
    AND zp1
    AND zp1,X
    AND $2000
    AND $2000,X
    AND $2000,Y
    AND (pointer,X)
    AND (pointer),Y

    ORA #$42
    ORA zp1
    ORA zp1,X
    ORA $2000
    ORA $2000,X
    ORA $2000,Y
    ORA (pointer,X)
    ORA (pointer),Y

    EOR #$42
    EOR zp1
    EOR zp1,X
    EOR $2000
    EOR $2000,X
    EOR $2000,Y
    EOR (pointer,X)
    EOR (pointer),Y

; *** Compare ***
compare:
    CMP #$42
    CMP zp1
    CMP zp1,X
    CMP $2000
    CMP $2000,X
    CMP $2000,Y
    CMP (pointer,X)
    CMP (pointer),Y

    CPX #$42
    CPX zp1
    CPX $2000

    CPY #$42
    CPY zp1
    CPY $2000

; *** Shifts/Rotates ***
shifts:
    ASL
    ASL zp1
    ASL zp1,X
    ASL $2000
    ASL $2000,X

    LSR
    LSR zp1
    LSR zp1,X
    LSR $2000
    LSR $2000,X

    ROL
    ROL zp1
    ROL zp1,X
    ROL $2000
    ROL $2000,X

    ROR
    ROR zp1
    ROR zp1,X
    ROR $2000
    ROR $2000,X

; *** Bit ***
bits:
    BIT zp1
    BIT $2000

; *** Jumps ***
jumps:
    JMP skip
    JMP (pointer)
    JSR subr
skip:
    RTS
subr:
    NOP
    RTS

; *** Branches ***
branches:
    BCC branch1
    BCS branch1
    BEQ branch1
    BMI branch1
    BNE branch1
    BPL branch1
    BVC branch1
    BVS branch1
branch1:

; *** Transfers ***
transfers:
    TAX
    TXA
    TAY
    TYA
    TSX
    TXS

; *** Stack ***
stack:
    PHA
    PLA
    PHP
    PLP

; *** Status ***
status:
    CLC
    SEC
    CLI
    SEI
    CLV
    CLD
    SED

; *** System ***
system:
    BRK
    NOP
    RTI

; Current address constant
HERE = *
    JMP HERE

; Next test
NEXT = *+1
    NOP
    JMP NEXT  ; Hopper til neste instruksjon (skipper NOP)

test_end:
    RTS
"#;

    let (mc, instructions) = assembler.assemble_full(test_code)?;

    // Print listing to console
    assembler.print_assembly_listing(&instructions);

    // Save binary and listing to files
    Assembler6502::write_bin(&mc, File::create("output.bin")?)?;
    assembler.save_listing(&instructions, "listing.txt").map_err(AsmError::Io)?;

    println!("\n✓ Machine code saved to output.bin");
    println!("✓ Listing saved to listing.txt");
    println!("✓ Total bytes: {}", mc.len());

    // Verify some key bytes
    assert_eq!(mc[0], 0x20, "First DCB should be $20");
    assert_eq!(mc[3], 0xA9, "LDA immediate opcode");
    assert_eq!(mc[4], 0x42, "LDA immediate value $42");

    println!("✓ Byte verification passed");

    Ok(())
}
