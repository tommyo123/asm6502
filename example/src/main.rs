use std::fs::File;
use asm6502::{Assembler6502, AsmError};

fn main() -> Result<(), AsmError> {
    let mut assembler = Assembler6502::new();

    // Demo / test program (synthetic coverage of modes; not necessarily meaningful at runtime)
    let test_code = r#"
*=$0080

    ; Zero page variables
    zp1:        DCB $20
    zp2:        DCB $30
    pointer:    DCB $40

; *** Load/Store ***
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

; *** Arithmetic ***
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

test_end:
    RTS
"#;

    {
        let (mc, instructions) = assembler.assemble_full(test_code)?;
        assembler.print_assembly_listing(&instructions);
        Assembler6502::write_bin(&mc, File::create("test_output.bin")?)?;
        assembler.save_listing(&instructions, "listing.txt").map_err(AsmError::Io)?;
        println!("Machine code saved to test_output.bin");
        println!("Listing saved to listing.txt");
        println!("Total bytes: {}", mc.len());
    }

    {
        let mc = assembler.assemble_bytes(test_code)?;
        Assembler6502::write_bin(&mc, File::create("out.bin")?)?;
    }

    Ok(())
}
