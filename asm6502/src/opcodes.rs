//! 6502 opcode tables and initialization

use std::collections::HashMap;

pub struct OpcodeTables {
    /// Base opcodes (implied/immediate modes)
    pub opcodes: HashMap<&'static str, u8>,
    /// Extended opcodes by mnemonic -> addressing mode -> opcode
    pub extended_opcodes: HashMap<&'static str, HashMap<&'static str, u8>>,
}

impl OpcodeTables {
    pub fn new() -> Self {
        let mut tables = Self {
            opcodes: HashMap::new(),
            extended_opcodes: HashMap::new(),
        };
        tables.init_opcodes();
        tables.init_address_modes();
        tables
    }

    fn init_opcodes(&mut self) {
        self.opcodes = HashMap::from([
            ("LDA", 0xA9), ("LDX", 0xA2), ("LDY", 0xA0),
            ("STA", 0x8D), ("STX", 0x8E), ("STY", 0x8C),
            ("ADC", 0x69), ("SBC", 0xE9),
            ("AND", 0x29), ("ORA", 0x09), ("EOR", 0x49),
            ("CMP", 0xC9), ("CPX", 0xE0), ("CPY", 0xC0),
            ("INC", 0xE6), ("INX", 0xE8), ("INY", 0xC8),
            ("DEC", 0xC6), ("DEX", 0xCA), ("DEY", 0x88),
            ("ASL", 0x0A), ("LSR", 0x4A), ("ROL", 0x2A), ("ROR", 0x6A),
            ("JMP", 0x4C), ("JSR", 0x20), ("RTS", 0x60), ("RTI", 0x40),
            ("BCC", 0x90), ("BCS", 0xB0), ("BEQ", 0xF0), ("BMI", 0x30),
            ("BNE", 0xD0), ("BPL", 0x10), ("BVC", 0x50), ("BVS", 0x70),
            ("CLC", 0x18), ("SEC", 0x38), ("CLD", 0xD8), ("SED", 0xF8),
            ("CLI", 0x58), ("SEI", 0x78), ("CLV", 0xB8),
            ("TAX", 0xAA), ("TXA", 0x8A), ("TAY", 0xA8), ("TYA", 0x98),
            ("TSX", 0xBA), ("TXS", 0x9A),
            ("PHA", 0x48), ("PLA", 0x68), ("PHP", 0x08), ("PLP", 0x28),
            ("BIT", 0x24), ("NOP", 0xEA), ("BRK", 0x00),
        ]);
    }

    fn init_address_modes(&mut self) {
        use std::iter::FromIterator;

        let lda: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0xA5), ("zeropage,X", 0xB5),
            ("absolute", 0xAD), ("absolute,X", 0xBD), ("absolute,Y", 0xB9),
            ("indirect,X", 0xA1), ("indirect,Y", 0xB1),
        ]);
        let ldx: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0xA6), ("zeropage,Y", 0xB6),
            ("absolute", 0xAE), ("absolute,Y", 0xBE),
        ]);
        let ldy: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0xA4), ("zeropage,X", 0xB4),
            ("absolute", 0xAC), ("absolute,X", 0xBC),
        ]);
        let sta: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x85), ("zeropage,X", 0x95),
            ("absolute", 0x8D), ("absolute,X", 0x9D), ("absolute,Y", 0x99),
            ("indirect,X", 0x81), ("indirect,Y", 0x91),
        ]);
        let stx: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x86), ("zeropage,Y", 0x96), ("absolute", 0x8E),
        ]);
        let sty: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x84), ("zeropage,X", 0x94), ("absolute", 0x8C),
        ]);
        let adc: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x65), ("zeropage,X", 0x75),
            ("absolute", 0x6D), ("absolute,X", 0x7D), ("absolute,Y", 0x79),
            ("indirect,X", 0x61), ("indirect,Y", 0x71),
        ]);
        let sbc: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0xE5), ("zeropage,X", 0xF5),
            ("absolute", 0xED), ("absolute,X", 0xFD), ("absolute,Y", 0xF9),
            ("indirect,X", 0xE1), ("indirect,Y", 0xF1),
        ]);
        let and_: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x25), ("zeropage,X", 0x35),
            ("absolute", 0x2D), ("absolute,X", 0x3D), ("absolute,Y", 0x39),
            ("indirect,X", 0x21), ("indirect,Y", 0x31),
        ]);
        let ora: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x05), ("zeropage,X", 0x15),
            ("absolute", 0x0D), ("absolute,X", 0x1D), ("absolute,Y", 0x19),
            ("indirect,X", 0x01), ("indirect,Y", 0x11),
        ]);
        let eor: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x45), ("zeropage,X", 0x55),
            ("absolute", 0x4D), ("absolute,X", 0x5D), ("absolute,Y", 0x59),
            ("indirect,X", 0x41), ("indirect,Y", 0x51),
        ]);
        let cmp: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0xC5), ("zeropage,X", 0xD5),
            ("absolute", 0xCD), ("absolute,X", 0xDD), ("absolute,Y", 0xD9),
            ("indirect,X", 0xC1), ("indirect,Y", 0xD1),
        ]);
        let cpx: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0xE4), ("absolute", 0xEC),
        ]);
        let cpy: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0xC4), ("absolute", 0xCC),
        ]);
        let bit: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x24), ("absolute", 0x2C),
        ]);
        let asl: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x06), ("zeropage,X", 0x16), ("absolute", 0x0E), ("absolute,X", 0x1E),
        ]);
        let lsr: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x46), ("zeropage,X", 0x56), ("absolute", 0x4E), ("absolute,X", 0x5E),
        ]);
        let rol: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x26), ("zeropage,X", 0x36), ("absolute", 0x2E), ("absolute,X", 0x3E),
        ]);
        let ror: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x66), ("zeropage,X", 0x76), ("absolute", 0x6E), ("absolute,X", 0x7E),
        ]);
        let dec: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0xC6), ("zeropage,X", 0xD6), ("absolute", 0xCE), ("absolute,X", 0xDE),
        ]);
        let inc: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0xE6), ("zeropage,X", 0xF6), ("absolute", 0xEE), ("absolute,X", 0xFE),
        ]);
        let jsr: HashMap<&'static str, u8> = HashMap::from_iter([
            ("absolute", 0x20),
        ]);

        self.extended_opcodes = HashMap::from([
            ("LDA", lda), ("LDX", ldx), ("LDY", ldy),
            ("STA", sta), ("STX", stx), ("STY", sty),
            ("ADC", adc), ("SBC", sbc),
            ("AND", and_), ("ORA", ora), ("EOR", eor),
            ("CMP", cmp), ("CPX", cpx), ("CPY", cpy),
            ("BIT", bit),
            ("ASL", asl), ("LSR", lsr), ("ROL", rol), ("ROR", ror),
            ("DEC", dec), ("INC", inc),
            ("JSR", jsr),
        ]);
    }
}

impl Default for OpcodeTables {
    fn default() -> Self {
        Self::new()
    }
}
