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
//!   - `*=$xxxx` – set program origin (ORG).
//!   - `DCB $nn ...` – define raw bytes.
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

use std::collections::{HashMap, HashSet};
use std::fs::File;
#[cfg(feature = "listing")]
use std::io::{self, Write};

#[derive(Clone, Debug)]
pub enum Item {
    Instruction { mnemonic: String, operand: Option<String> },
    Label(String),
    Data(Vec<u8>),
    Org(u16),
}

#[derive(Debug)]
pub enum AsmError {
    Asm(String),
    Io(std::io::Error),
}
impl From<std::io::Error> for AsmError { fn from(e: std::io::Error) -> Self { AsmError::Io(e) } }

#[derive(Copy, Clone, PartialEq, Eq)]
enum AddrOverride { Auto, ForceZp, ForceAbs }

pub struct Assembler6502 {
    opcodes: HashMap<&'static str, u8>,
    extended_opcodes: HashMap<&'static str, HashMap<&'static str, u8>>, // mnemonic -> mode -> opcode
    start_address: u16,
    labels: HashMap<String, u16>,
    #[allow(dead_code)]
    zp_labels: HashSet<String>,
}

impl Default for Assembler6502 { fn default() -> Self { Self::new() } }

impl Assembler6502 {
    pub fn new() -> Self {
        let mut asm = Self {
            opcodes: HashMap::new(),
            extended_opcodes: HashMap::new(),
            start_address: 0x0080,
            labels: HashMap::new(),
            zp_labels: HashSet::new(),
        };
        asm.init_opcodes();
        asm.init_address_modes();
        asm
    }

    // ===== Public minimal API =====
    pub fn assemble_bytes(&mut self, src: &str) -> Result<Vec<u8>, AsmError> {
        let (bytes, _items) = self.assemble(src).map_err(AsmError::Asm)?;
        Ok(bytes)
    }
    pub fn assemble_into(&mut self, src: &str, out: &mut Vec<u8>) -> Result<(), AsmError> {
        out.clear();
        let (bytes, _items) = self.assemble(src).map_err(AsmError::Asm)?;
        out.extend_from_slice(&bytes);
        Ok(())
    }
    pub fn assemble_full(&mut self, src: &str) -> Result<(Vec<u8>, Vec<Item>), AsmError> {
        self.assemble(src).map_err(AsmError::Asm)
    }
    pub fn set_origin(&mut self, addr: u16) { self.start_address = addr; }
    pub fn origin(&self) -> u16 { self.start_address }
    pub fn symbols(&self) -> &HashMap<String, u16> { &self.labels }
    pub fn lookup(&self, name: &str) -> Option<u16> { self.labels.get(name).copied() }
    pub fn assemble_with_symbols(&mut self, src: &str) -> Result<(Vec<u8>, HashMap<String,u16>), AsmError> {
        let (b, _) = self.assemble(src).map_err(AsmError::Asm)?;
        Ok((b, self.labels.clone()))
    }
    pub fn assemble_with_addr_map(&mut self, src: &str) -> Result<(Vec<u8>, Vec<(usize,u16)>), AsmError> {
        let (bytes, items) = self.assemble(src).map_err(AsmError::Asm)?;
        let mut map = Vec::new();
        let mut pc = self.start_address;
        let mut idx = 0usize;
        for it in items.iter() {
            match it {
                Item::Instruction { mnemonic, operand } => {
                    let b = self.assemble_instruction(mnemonic, operand.as_deref(), pc).map_err(AsmError::Asm)?;
                    for _ in 0..b.len() { map.push((idx, pc)); idx += 1; pc = pc.wrapping_add(1); }
                }
                Item::Data(ds) => { for _ in 0..ds.len() { map.push((idx, pc)); idx += 1; pc = pc.wrapping_add(1); } }
                Item::Org(a) => { pc = *a; }
                Item::Label(_) => {}
            }
        }
        Ok((bytes, map))
    }
    pub fn write_bin<W: std::io::Write>(bytes: &[u8], mut w: W) -> std::io::Result<()> { w.write_all(bytes) }
    pub fn reset(&mut self) { self.labels.clear(); self.start_address = 0x0080; }

    // ===== Internals =====
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
        let mut lda: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0xA5), ("zeropage,X", 0xB5),
            ("absolute", 0xAD), ("absolute,X", 0xBD), ("absolute,Y", 0xB9),
            ("indirect,X", 0xA1), ("indirect,Y", 0xB1),
        ]);
        let mut ldx: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0xA6), ("zeropage,Y", 0xB6),
            ("absolute", 0xAE), ("absolute,Y", 0xBE),
        ]);
        let mut ldy: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0xA4), ("zeropage,X", 0xB4),
            ("absolute", 0xAC), ("absolute,X", 0xBC),
        ]);
        let mut sta: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x85), ("zeropage,X", 0x95),
            ("absolute", 0x8D), ("absolute,X", 0x9D), ("absolute,Y", 0x99),
            ("indirect,X", 0x81), ("indirect,Y", 0x91),
        ]);
        let mut stx: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x86), ("zeropage,Y", 0x96), ("absolute", 0x8E),
        ]);
        let mut sty: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x84), ("zeropage,X", 0x94), ("absolute", 0x8C),
        ]);
        let mut adc: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x65), ("zeropage,X", 0x75),
            ("absolute", 0x6D), ("absolute,X", 0x7D), ("absolute,Y", 0x79),
            ("indirect,X", 0x61), ("indirect,Y", 0x71),
        ]);
        let mut sbc: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0xE5), ("zeropage,X", 0xF5),
            ("absolute", 0xED), ("absolute,X", 0xFD), ("absolute,Y", 0xF9),
            ("indirect,X", 0xE1), ("indirect,Y", 0xF1),
        ]);
        let mut and_: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x25), ("zeropage,X", 0x35),
            ("absolute", 0x2D), ("absolute,X", 0x3D), ("absolute,Y", 0x39),
            ("indirect,X", 0x21), ("indirect,Y", 0x31),
        ]);
        let mut ora: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x05), ("zeropage,X", 0x15),
            ("absolute", 0x0D), ("absolute,X", 0x1D), ("absolute,Y", 0x19),
            ("indirect,X", 0x01), ("indirect,Y", 0x11),
        ]);
        let mut eor: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x45), ("zeropage,X", 0x55),
            ("absolute", 0x4D), ("absolute,X", 0x5D), ("absolute,Y", 0x59),
            ("indirect,X", 0x41), ("indirect,Y", 0x51),
        ]);
        let mut cmp: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0xC5), ("zeropage,X", 0xD5),
            ("absolute", 0xCD), ("absolute,X", 0xDD), ("absolute,Y", 0xD9),
            ("indirect,X", 0xC1), ("indirect,Y", 0xD1),
        ]);
        let mut cpx: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0xE4), ("absolute", 0xEC),
        ]);
        let mut cpy: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0xC4), ("absolute", 0xCC),
        ]);
        let mut bit: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x24), ("absolute", 0x2C),
        ]);
        let mut asl: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x06), ("zeropage,X", 0x16), ("absolute", 0x0E), ("absolute,X", 0x1E),
        ]);
        let mut lsr: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x46), ("zeropage,X", 0x56), ("absolute", 0x4E), ("absolute,X", 0x5E),
        ]);
        let mut rol: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x26), ("zeropage,X", 0x36), ("absolute", 0x2E), ("absolute,X", 0x3E),
        ]);
        let mut ror: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0x66), ("zeropage,X", 0x76), ("absolute", 0x6E), ("absolute,X", 0x7E),
        ]);
        let mut dec: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0xC6), ("zeropage,X", 0xD6), ("absolute", 0xCE), ("absolute,X", 0xDE),
        ]);
        let mut inc: HashMap<&'static str, u8> = HashMap::from_iter([
            ("zeropage", 0xE6), ("zeropage,X", 0xF6), ("absolute", 0xEE), ("absolute,X", 0xFE),
        ]);
        let mut jsr: HashMap<&'static str, u8> = HashMap::from_iter([
            ("absolute", 0x20),
        ]);

        self.extended_opcodes = HashMap::from([
            ("LDA", lda.drain().collect()),
            ("LDX", ldx.drain().collect()),
            ("LDY", ldy.drain().collect()),
            ("STA", sta.drain().collect()),
            ("STX", stx.drain().collect()),
            ("STY", sty.drain().collect()),
            ("ADC", adc.drain().collect()),
            ("SBC", sbc.drain().collect()),
            ("AND", and_.drain().collect()),
            ("ORA", ora.drain().collect()),
            ("EOR", eor.drain().collect()),
            ("CMP", cmp.drain().collect()),
            ("CPX", cpx.drain().collect()),
            ("CPY", cpy.drain().collect()),
            ("BIT", bit.drain().collect()),
            ("ASL", asl.drain().collect()),
            ("LSR", lsr.drain().collect()),
            ("ROL", rol.drain().collect()),
            ("ROR", ror.drain().collect()),
            ("DEC", dec.drain().collect()),
            ("INC", inc.drain().collect()),
            ("JSR", jsr.drain().collect()),
        ]);
    }

    fn get_address_value(&self, operand: &str) -> Result<u16, String> {
        if let Some(rest) = operand.strip_prefix('$') {
            u16::from_str_radix(rest, 16).map_err(|_| format!("Invalid hex: {}", operand))
        } else if let Some(&addr) = self.labels.get(operand) { Ok(addr) } else {
            Err(format!("Invalid address or label: {}", operand))
        }
    }

    fn handle_jump(&self, operand: &str) -> Result<Vec<u8>, String> {
        if operand.starts_with('(') && operand.ends_with(')') {
            let inner = &operand[1..operand.len()-1];
            let value = self.get_address_value(inner)?;
            return Ok(vec![0x6C, (value & 0xFF) as u8, (value >> 8) as u8]);
        }
        let value = self.get_address_value(operand)?;
        Ok(vec![0x4C, (value & 0xFF) as u8, (value >> 8) as u8])
    }

    fn handle_subroutine(&self, operand: &str) -> Result<Vec<u8>, String> {
        let value = self.get_address_value(operand)?;
        Ok(vec![0x20, (value & 0xFF) as u8, (value >> 8) as u8])
    }

    fn handle_branch(&self, mnemonic: &str, operand: &str, current_address: u16) -> Result<Vec<u8>, String> {
        let target = *self.labels.get(operand).ok_or_else(|| format!("Undefined label: {}", operand))?;
        let offset = target as i32 - (current_address as i32 + 2);
        if offset < -128 || offset > 127 {
            return Err(format!(
                "Branch offset out of range: {}. Target: ${:04X}, Current: ${:04X}", offset, target, current_address));
        }
        let opcode = *self.opcodes.get(mnemonic).unwrap();
        Ok(vec![opcode, (offset as i8) as u8])
    }

    pub fn parse_source(&self, source: &str) -> Result<Vec<Item>, String> {
        let mut instructions = Vec::new();
        for raw in source.lines() {
            let line = raw.split(';').next().unwrap_or("").trim().to_string();
            if line.is_empty() { continue; }
            if let Some(parsed) = self.parse_line(&line)? {
                match parsed { Either::Many(list) => instructions.extend(list), Either::One(item) => instructions.push(item) }
            }
        }
        Ok(instructions)
    }

    fn parse_line(&self, line: &str) -> Result<Option<Either<Item>>, String> {
        let l = line.split(';').next().unwrap_or("").trim();
        if l.is_empty() { return Ok(None); }

        if l.contains(':') && l.contains("DCB") {
            let mut parts = l.split(':');
            let label = parts.next().unwrap().trim().to_string();
            let rest = parts.next().unwrap_or("").trim();
            if rest.starts_with("DCB") {
                let data_vals: Vec<u8> = rest[3..]
                    .split_whitespace()
                    .filter_map(|b| b.strip_prefix('$'))
                    .map(|h| u8::from_str_radix(h, 16).map_err(|e| e.to_string()))
                    .collect::<Result<_, _>>()?;
                return Ok(Some(Either::Many(vec![ Item::Label(label), Item::Data(data_vals), ])));
            }
        }

        if l.ends_with(':') { return Ok(Some(Either::One(Item::Label(l[..l.len()-1].to_string())))); }

        if let Some(rest) = l.strip_prefix("*=") {
            let mut s = rest.trim();
            if let Some(hex) = s.strip_prefix('$') { s = hex; }
            let addr = u16::from_str_radix(s, 16).map_err(|e| e.to_string())?;
            return Ok(Some(Either::One(Item::Org(addr))));
        }

        if l.starts_with("DCB") {
            let data: Vec<u8> = l[3..]
                .split_whitespace()
                .filter_map(|b| b.strip_prefix('$'))
                .map(|h| u8::from_str_radix(h, 16).map_err(|e| e.to_string()))
                .collect::<Result<_, _>>()?;
            return Ok(Some(Either::One(Item::Data(data))));
        }

        let parts: Vec<&str> = l.split_whitespace().collect();
        match parts.len() {
            1 => Ok(Some(Either::One(Item::Instruction { mnemonic: parts[0].to_string(), operand: None }))),
            2 => Ok(Some(Either::One(Item::Instruction { mnemonic: parts[0].to_string(), operand: Some(parts[1].to_string()) }))),
            _ => Err(format!("Invalid line: {}", l)),
        }
    }

    fn instruction_size(&self, inst: &Item) -> usize {
        match inst {
            Item::Instruction { mnemonic, operand } => {
                if let Ok(bytes) = self.assemble_instruction(mnemonic, operand.as_deref(), 0) { return bytes.len(); }
                let m = mnemonic.as_str();
                if self.opcodes.contains_key(m) && operand.is_none() { return 1; }
                if self.opcodes.contains_key(m) && operand.as_ref().map(|s| s.starts_with('#')).unwrap_or(false) { return 2; }
                if Self::is_branch(m) { return 2; }
                3
            }
            Item::Data(data) => data.len(),
            Item::Org(_) | Item::Label(_) => 0,
        }
    }

    fn calculate_branch_distance(&self, from_addr: u16, to_addr: u16) -> (i16, bool) {
        let offset = to_addr as i32 - (from_addr as i32 + 2);
        (offset as i16, (-128..=127).contains(&(offset as i16)))
    }
    fn is_branch(m: &str) -> bool { matches!(m, "BCC"|"BCS"|"BEQ"|"BMI"|"BNE"|"BPL"|"BVC"|"BVS") }
    fn count_branches(&self, items: &[Item]) -> usize {
        items.iter().filter(|it| match it {
            Item::Instruction { mnemonic, .. } => Self::is_branch(mnemonic.as_str()),
            _ => false,
        }).count()
    }

    pub fn fix_long_branches(&mut self, instructions: &[Item]) -> (Vec<Item>, bool) {
        // First pass: collect label addresses based on current sizes
        let mut fixed: Vec<Item> = Vec::new();
        let mut current_address = self.start_address;
        let mut modified = false;
        for inst in instructions.iter() {
            match inst {
                Item::Label(name) => { self.labels.insert(name.clone(), current_address); },
                _ => { current_address = current_address.wrapping_add(self.instruction_size(inst) as u16); }
            }
        }
        // Second pass: emit with expansion if needed
        current_address = self.start_address;
        let mut unique_counter = 0u32;
        for inst in instructions.iter() {
            if let Item::Instruction { mnemonic, operand } = inst {
                if Self::is_branch(mnemonic.as_str()) {
                    if let Some(op) = operand {
                        if let Some(&target_addr) = self.labels.get(op) {
                            let (_, in_range) = self.calculate_branch_distance(current_address, target_addr);
                            if !in_range {
                                let skip_label = format!("__skip_{}", unique_counter); unique_counter += 1;
                                fixed.push(Item::Instruction { mnemonic: mnemonic.clone(), operand: Some(skip_label.clone()) });
                                fixed.push(Item::Instruction { mnemonic: "JMP".to_string(), operand: Some(op.clone()) });
                                fixed.push(Item::Label(skip_label));
                                modified = true;
                                current_address = current_address.wrapping_add(5); // branch (2) + jmp abs (3)
                                continue;
                            }
                        }
                    }
                }
            }
            fixed.push(inst.clone());
            if !matches!(inst, Item::Label(_)) { current_address = current_address.wrapping_add(self.instruction_size(inst) as u16); }
        }
        (fixed, modified)
    }

    fn assemble_instruction(&self, mnemonic: &str, operand: Option<&str>, current_address: u16) -> Result<Vec<u8>, String> {
        // Implied/accumulator form: emit single-byte opcode when no operand is present
        if operand.is_none() {
            if let Some(&op) = self.opcodes.get(mnemonic) { return Ok(vec![op]); }
            return Err(format!("Unknown mnemonic: {}", mnemonic));
        }

        // Keep raw operand to support simple ZP/ABS forcing prefixes
        let operand_raw = operand.unwrap();
        // Force addressing mode with operand prefixes:
        //   "<$80" => force Zero Page (2-byte instruction if supported)
        //   ">$80" => force Absolute   (3-byte instruction)
        let (operand, mode_override) = if let Some(s) = operand_raw.strip_prefix('<') {
            (s.trim(), AddrOverride::ForceZp)
        } else if let Some(s) = operand_raw.strip_prefix('>') {
            (s.trim(), AddrOverride::ForceAbs)
        } else {
            (operand_raw, AddrOverride::Auto)
        };

        // Jumps, subroutines and branches have custom handlers
        if mnemonic == "JMP" { return self.handle_jump(operand); }
        if mnemonic == "JSR" { return self.handle_subroutine(operand); }
        if Self::is_branch(mnemonic) { return self.handle_branch(mnemonic, operand, current_address); }

        // Immediate mode: #$nn
        if let Some(rest) = operand.strip_prefix('#') {
            let rest = rest.strip_prefix('$').unwrap_or(rest);
            let value = u8::from_str_radix(rest, 16).map_err(|e| e.to_string())?;
            return Ok(vec![*self.opcodes.get(mnemonic).ok_or_else(|| format!("Unknown mnemonic: {}", mnemonic))? , value]);
        }

        // Indirect modes: (zp,X) and (zp),Y
        if operand.starts_with('(') {
            // (addr),Y — note: ')' comes before ',Y' for indirect Y
            if operand.contains("),Y") {
                let inner = operand
                    .strip_prefix('(')
                    .and_then(|s| s.split("),Y").next())
                    .unwrap_or("")
                    .trim();
                let val = self.get_address_value(inner)?;
                let code = self.extended_opcodes.get(mnemonic).and_then(|m| m.get("indirect,Y"))
                    .ok_or_else(|| format!("Unsupported mode for {}", mnemonic))?;
                return Ok(vec![*code, (val & 0xFF) as u8]);
            }
            // (addr,X)
            if operand.ends_with(')') {
                let inside = &operand[1..operand.len()-1];
                let mut parts = inside.split(',').map(|s| s.trim());
                let a = parts.next().unwrap_or("");
                let idx = parts.next().unwrap_or("");
                if idx.eq_ignore_ascii_case("X") {
                    let val = self.get_address_value(a)?;
                    let code = self.extended_opcodes.get(mnemonic).and_then(|m| m.get("indirect,X"))
                        .ok_or_else(|| format!("Unsupported mode for {}", mnemonic))?;
                    return Ok(vec![*code, (val & 0xFF) as u8]);
                }
            }
            return Err("Invalid indirect addressing mode".to_string());
        }

        // Indexed absolute/zeropage: $addr,X or $addr,Y (or with labels)
        if let Some((addr_part, idx)) = operand.split_once(',') {
            let addr_part = addr_part.trim();
            let idx = idx.trim();
            let val = self.get_address_value(addr_part)?;
            let force_zp  = mode_override == AddrOverride::ForceZp;
            let force_abs = mode_override == AddrOverride::ForceAbs;
            let is_zp = val < 0x100;
            let mode_zp = format!("zeropage,{}", idx);
            let mode_abs = format!("absolute,{}", idx);
            // Prefer ZP if available and not forced to ABS; or explicitly forced to ZP
            if (is_zp && !force_abs) || force_zp {
                if let Some(code) = self.extended_opcodes.get(mnemonic).and_then(|m| m.get(mode_zp.as_str())) {
                    return Ok(vec![*code, (val & 0xFF) as u8]);
                }
            }
            // Fall back to ABS indexed
            let code = self.extended_opcodes.get(mnemonic).and_then(|m| m.get(mode_abs.as_str()))
                .ok_or_else(|| format!("Unsupported mode for {}", mnemonic))?;
            return Ok(vec![*code, (val & 0xFF) as u8, (val >> 8) as u8]);
        }

        // Plain absolute/zeropage by value or label
        let val = self.get_address_value(operand)?;
        let force_zp  = mode_override == AddrOverride::ForceZp;
        let force_abs = mode_override == AddrOverride::ForceAbs;
        if (val < 0x100 && !force_abs) || force_zp {
            if let Some(code) = self.extended_opcodes.get(mnemonic).and_then(|m| m.get("zeropage")) {
                return Ok(vec![*code, (val & 0xFF) as u8]);
            }
        }
        let code = self.extended_opcodes.get(mnemonic).and_then(|m| m.get("absolute"))
            .ok_or_else(|| format!("Unsupported mode for {}", mnemonic))?;
        Ok(vec![*code, (val & 0xFF) as u8, (val >> 8) as u8])
    }

    fn assemble(&mut self, code: &str) -> Result<(Vec<u8>, Vec<Item>), String> {
        let mut instructions = self.parse_source(code)?;
        // Adaptive pass limit based on the number of branch instructions. Each branch can be
        // expanded at most once into "BRANCH skip" + "JMP target". This makes the pass count
        // linear in the number of branches, and avoids an arbitrary fixed cap.
        let mut guard = self.count_branches(&instructions) + 2; // small slack
        loop {
            self.labels.clear();
            let (fixed, modified) = self.fix_long_branches(&instructions);
            instructions = fixed;
            if !modified { break; }
            if guard == 0 { return Err("Long-branch fix didn't converge".to_string()); }
            guard -= 1;
        }
        let mut machine: Vec<u8> = Vec::new();
        let mut current_address = self.start_address;
        self.labels.clear();
        // First, compute final label addresses
        for inst in instructions.iter() {
            match inst {
                Item::Label(name) => { self.labels.insert(name.clone(), current_address); },
                Item::Org(addr) => { current_address = *addr; },
                _ => { current_address = current_address.wrapping_add(self.instruction_size(inst) as u16); }
            }
        }
        // Second, emit bytes
        current_address = self.start_address;
        for inst in instructions.iter() {
            match inst {
                Item::Label(_) => {},
                Item::Org(addr) => { current_address = *addr; },
                Item::Data(bytes) => { machine.extend_from_slice(bytes); current_address = current_address.wrapping_add(bytes.len() as u16); },
                Item::Instruction { mnemonic, operand } => {
                    let bytes = self.assemble_instruction(mnemonic, operand.as_deref(), current_address)?;
                    current_address = current_address.wrapping_add(bytes.len() as u16);
                    machine.extend_from_slice(&bytes);
                }
            }
        }
        Ok((machine, instructions))
    }

    #[cfg(feature = "listing")]
    pub fn print_assembly_listing(&self, instructions: &[Item]) {
        let mut current_address = self.start_address;
        println!("\nAssembly Listing:");
        println!("Address:  Machine Code  Assembly");
        println!("{}", "-".repeat(50));
        for inst in instructions.iter() {
            match inst {
                Item::Label(name) => { println!("${:04X}:          {}:", current_address, name); }
                Item::Instruction { mnemonic, operand } => {
                    let size = self.instruction_size(inst);
                    let code_bytes = self.assemble_instruction(mnemonic, operand.as_deref(), current_address).unwrap_or_else(|_| vec![]);
                    let hex_bytes = code_bytes.iter().map(|b| format!("${:02X}", b)).collect::<Vec<_>>().join(" ");
                    let hex_padded = format!("{:<12}", hex_bytes);
                    let op_str = operand.clone().unwrap_or_default();
                    println!("${:04X}: {} {} {}", current_address, hex_padded, mnemonic, op_str);
                    current_address = current_address.wrapping_add(size as u16);
                }
                Item::Org(addr) => { println!("${:04X}:          *=${:04X}", current_address, addr); current_address = *addr; }
                Item::Data(bytes) => {
                    let hex_data = bytes.iter().map(|b| format!("${:02X}", b)).collect::<Vec<_>>().join(" ");
                    let hex_padded = format!("{:<12}", hex_data.clone());
                    println!("${:04X}: {} DCB {}", current_address, hex_padded, hex_data);
                    current_address = current_address.wrapping_add(bytes.len() as u16);
                }
            }
        }
    }

    #[cfg(feature = "listing")]
    pub fn save_listing(&self, instructions: &[Item], filename: &str) -> io::Result<()> {
        let mut f = File::create(filename)?;
        writeln!(f, "Assembly Listing:")?;
        writeln!(f, "Address:  Machine Code  Assembly")?;
        writeln!(f, "{}", "-".repeat(50))?;
        let mut current_address = self.start_address;
        for inst in instructions.iter() {
            match inst {
                Item::Label(name) => { writeln!(f, "${:04X}:          {}:", current_address, name)?; }
                Item::Instruction { mnemonic, operand } => {
                    let size = self.instruction_size(inst);
                    let code_bytes = self.assemble_instruction(mnemonic, operand.as_deref(), current_address).unwrap_or_default();
                    let hex_bytes = code_bytes.iter().map(|b| format!("${:02X}", b)).collect::<Vec<_>>().join(" ");
                    let hex_padded = format!("{:<12}", hex_bytes);
                    let op_str = operand.clone().unwrap_or_default();
                    writeln!(f, "${:04X}: {} {} {}", current_address, hex_padded, mnemonic, op_str)?;
                    current_address = current_address.wrapping_add(size as u16);
                }
                Item::Org(addr) => { writeln!(f, "${:04X}:          *=${:04X}", current_address, addr)?; current_address = *addr; }
                Item::Data(bytes) => {
                    let hex_data = bytes.iter().map(|b| format!("${:02X}", b)).collect::<Vec<_>>().join(" ");
                    let hex_padded = format!("{:<12}", hex_data.clone());
                    writeln!(f, "${:04X}: {} DCB {}", current_address, hex_padded, hex_data)?;
                    current_address = current_address.wrapping_add(bytes.len() as u16);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum Either<T> { One(T), Many(Vec<T>) }
