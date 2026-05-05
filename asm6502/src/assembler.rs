//! Main assembler implementation

use std::fs;

#[cfg(feature = "listing")]
use std::fs::File;
#[cfg(feature = "listing")]
use std::io::{self, Write};

use crate::error::AsmError;
use crate::opcodes::OpcodeTables;
use crate::symbol::SymbolTable;
use crate::parser::{parse_source, parse_line, Either, ExpressionParser};
use crate::addressing::{invert_branch, parse_addr_override, is_branch, AddrOverride};
use crate::eval::ExpressionEvaluator;
use crate::reserved::ReservedRange;

// Re-export Item for public API
pub use crate::parser::lexer::Item;

pub struct Assembler6502 {
    opcodes: OpcodeTables,
    symbols: SymbolTable,
    start_address: u16,
    skip_label_counter: u32,
    reserved_ranges: Vec<ReservedRange>,
}

impl Default for Assembler6502 {
    fn default() -> Self {
        Self::new()
    }
}

impl Assembler6502 {
    pub fn new() -> Self {
        Self {
            opcodes: OpcodeTables::new(),
            symbols: SymbolTable::new(),
            start_address: 0x0080,
            skip_label_counter: 0,
            reserved_ranges: Vec::new(),
        }
    }

    // ===== Public API =====

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

    pub fn set_origin(&mut self, addr: u16) {
        self.start_address = addr;
    }

    pub fn origin(&self) -> u16 {
        self.start_address
    }

    pub fn symbols(&self) -> &std::collections::HashMap<String, u16> {
        self.symbols.labels()
    }

    pub fn lookup(&self, name: &str) -> Option<u16> {
        self.symbols.get(name)
    }

    pub fn assemble_with_symbols(
        &mut self,
        src: &str,
    ) -> Result<(Vec<u8>, std::collections::HashMap<String, u16>), AsmError> {
        let (b, _) = self.assemble(src).map_err(AsmError::Asm)?;
        Ok((b, self.symbols.clone_labels()))
    }

    pub fn assemble_with_addr_map(
        &mut self,
        src: &str,
    ) -> Result<(Vec<u8>, Vec<(usize, u16)>), AsmError> {
        let (bytes, items) = self.assemble(src).map_err(AsmError::Asm)?;
        let mut map = Vec::new();
        let mut pc = self.start_address;
        let mut idx = 0usize;
        for it in items.iter() {
            match it {
                Item::Instruction { mnemonic, operand } => {
                    let b = self
                        .assemble_instruction(mnemonic, operand.as_deref(), pc)
                        .map_err(AsmError::Asm)?;
                    for _ in 0..b.len() {
                        map.push((idx, pc));
                        idx += 1;
                        pc = pc.wrapping_add(1);
                    }
                }
                Item::Data(exprs) => {
                    let eval = ExpressionEvaluator::new(&self.symbols, pc);
                    for expr in exprs {
                        eval.evaluate_u16(expr).map_err(AsmError::Asm)?;
                        map.push((idx, pc));
                        idx += 1;
                        pc = pc.wrapping_add(1);
                    }
                }
                Item::Words(exprs) => {
                    let eval = ExpressionEvaluator::new(&self.symbols, pc);
                    for expr in exprs {
                        eval.evaluate_u16(expr).map_err(AsmError::Asm)?;
                        map.push((idx, pc));
                        idx += 1;
                        pc = pc.wrapping_add(1);
                        map.push((idx, pc));
                        idx += 1;
                        pc = pc.wrapping_add(1);
                    }
                }
                Item::String(s) => {
                    for _ in s.bytes() {
                        map.push((idx, pc));
                        idx += 1;
                        pc = pc.wrapping_add(1);
                    }
                }
                Item::IncBin(filename) => {
                    if let Ok(bytes) = fs::read(filename) {
                        for _ in bytes {
                            map.push((idx, pc));
                            idx += 1;
                            pc = pc.wrapping_add(1);
                        }
                    }
                }
                Item::Org(expr) => {
                    let eval = ExpressionEvaluator::new(&self.symbols, pc);
                    pc = eval.evaluate_u16(expr).map_err(AsmError::Asm)?;
                }
                Item::Pad(n) => {
                    for _ in 0..*n {
                        map.push((idx, pc));
                        idx += 1;
                        pc = pc.wrapping_add(1);
                    }
                }
                Item::Label(_) | Item::Constant(_, _) => {}
            }
        }
        Ok((bytes, map))
    }

    pub fn write_bin<W: std::io::Write>(bytes: &[u8], mut w: W) -> std::io::Result<()> {
        w.write_all(bytes)
    }

    pub fn reset(&mut self) {
        self.symbols.clear();
        self.start_address = 0x0080;
        self.skip_label_counter = 0;
    }

    // ===== Reserved memory ranges =====

    /// Mark `[start, end]` (inclusive) as reserved. The assembler will
    /// emit a `JMP <end+1>` plus `$00`-fill instead of placing code there.
    /// Returns an error if the range is malformed, overlaps an existing
    /// reserved range, or sits closer than 3 bytes (the JMP size) to one.
    pub fn add_reserved_range(&mut self, start: u16, end: u16) -> Result<(), AsmError> {
        if start > end {
            return Err(AsmError::Asm(format!(
                "Invalid reserved range: ${:04X}-${:04X} (start > end)",
                start, end
            )));
        }
        if end == 0xFFFF {
            return Err(AsmError::Asm(format!(
                "Reserved range ${:04X}-${:04X} extends to end of address space; nothing to JMP to",
                start, end
            )));
        }
        for r in &self.reserved_ranges {
            if start <= r.end && end >= r.start {
                return Err(AsmError::Asm(format!(
                    "Reserved range ${:04X}-${:04X} overlaps existing ${:04X}-${:04X}",
                    start, end, r.start, r.end
                )));
            }
            let (lo, hi) = if start < r.start { (end, r.start) } else { (r.end, start) };
            let gap = hi as i32 - lo as i32 - 1;
            if gap < 3 {
                return Err(AsmError::Asm(format!(
                    "Reserved range ${:04X}-${:04X} sits {} bytes from ${:04X}-${:04X}; need >= 3 bytes between ranges for the JMP",
                    start, end, gap, r.start, r.end
                )));
            }
        }
        self.reserved_ranges.push(ReservedRange { start, end });
        self.reserved_ranges.sort_by_key(|r| r.start);
        Ok(())
    }

    pub fn clear_reserved_ranges(&mut self) {
        self.reserved_ranges.clear();
    }

    pub fn reserved_ranges(&self) -> &[ReservedRange] {
        &self.reserved_ranges
    }

    // ===== Parsing =====

    pub fn parse_source(&self, source: &str) -> Result<Vec<Item>, String> {
        parse_source(source)
    }

    #[allow(dead_code)]
    fn parse_line(&self, line: &str) -> Result<Option<Either<Item>>, String> {
        parse_line(line)
    }

    // ===== Assembly core =====

    fn assemble(&mut self, code: &str) -> Result<(Vec<u8>, Vec<Item>), String> {
        let mut instructions = self.parse_source(code)?;
        self.skip_label_counter = 0;

        // Adaptive pass limit. Both reserved-range insertion and long-branch
        // expansion only ever ADD items, so convergence is guaranteed; the
        // bound just covers worst-case interleaving.
        let mut guard = self.count_branches(&instructions) + self.reserved_ranges.len() + 4;
        let mut iteration = 0;
        loop {
            let (after_reserved, mod_reserved) = self.apply_reserved_ranges(&instructions)?;
            instructions = after_reserved;

            self.symbols.clear();
            let (fixed, mod_branch) = self.fix_long_branches(&instructions);
            instructions = fixed;

            if !mod_reserved && !mod_branch {
                break;
            }
            iteration += 1;
            if guard == 0 {
                // Collect information about problematic branches
                let mut problematic_branches = Vec::new();
                let mut current_address = self.start_address;

                for inst in instructions.iter() {
                    if let Item::Instruction { mnemonic, operand } = inst {
                        if is_branch(mnemonic.as_str()) {
                            if let Some(target) = operand {
                                if let Some(target_addr) = self.symbols.get(target) {
                                    let offset = target_addr as i32 - (current_address as i32 + 2);
                                    if offset < -128 || offset > 127 {
                                        problematic_branches.push(format!(
                                            "${:04X}: {} {} (offset: {}, target: ${:04X})",
                                            current_address, mnemonic, target, offset, target_addr
                                        ));
                                    }
                                }
                            }
                        }
                        if let Ok(size) = self.instruction_size(inst, current_address) {
                            current_address = current_address.wrapping_add(size as u16);
                        }
                    } else if !matches!(inst, Item::Label(_)) {
                        if let Ok(size) = self.instruction_size(inst, current_address) {
                            current_address = current_address.wrapping_add(size as u16);
                        }
                    }
                }

                if problematic_branches.is_empty() {
                    return Err(format!(
                        "Long-branch fix didn't converge after {} iterations (no obvious problematic branches found)",
                        iteration
                    ));
                } else {
                    return Err(format!(
                        "Long-branch fix didn't converge after {} iterations. Problematic branches:\n  {}",
                        iteration,
                        problematic_branches.join("\n  ")
                    ));
                }
            }
            guard -= 1;
        }

        let mut machine: Vec<u8> = Vec::new();
        let mut current_address = self.start_address;

        // First pass: compute label addresses and evaluate constants
        self.symbols.clear();
        for inst in instructions.iter() {
            match inst {
                Item::Label(name) => {
                    self.symbols.insert(name.clone(), current_address);
                }
                Item::Constant(name, expr) => {
                    // Evaluate constant and add to symbol table
                    let eval = ExpressionEvaluator::new(&self.symbols, current_address);
                    let value = eval.evaluate_u16(expr)
                        .map_err(|e| format!("Constant '{}': {}", name, e))?;
                    self.symbols.insert(name.clone(), value);
                }
                Item::Org(expr) => {
                    let eval = ExpressionEvaluator::new(&self.symbols, current_address);
                    current_address = eval.evaluate_u16(expr)
                        .map_err(|e| format!("ORG directive: {}", e))?;
                }
                _ => {
                    current_address =
                        current_address.wrapping_add(self.instruction_size(inst, current_address)? as u16);
                }
            }
        }

        // Second pass: emit bytes
        current_address = self.start_address;
        for inst in instructions.iter() {
            match inst {
                Item::Label(_) => {}
                Item::Constant(_, _) => {}  // Constants don't emit bytes
                Item::Org(expr) => {
                    let eval = ExpressionEvaluator::new(&self.symbols, current_address);
                    current_address = eval.evaluate_u16(expr)
                        .map_err(|e| format!("ORG directive: {}", e))?;
                }
                Item::Data(exprs) => {
                    let eval = ExpressionEvaluator::new(&self.symbols, current_address);
                    for expr in exprs {
                        let val = eval.evaluate_u16(expr)
                            .map_err(|e| format!(".byte directive at ${:04X}: {}", current_address, e))?;
                        machine.push((val & 0xFF) as u8);
                        current_address = current_address.wrapping_add(1);
                    }
                }
                Item::Words(exprs) => {
                    let eval = ExpressionEvaluator::new(&self.symbols, current_address);
                    for expr in exprs {
                        let val = eval.evaluate_u16(expr)
                            .map_err(|e| format!(".word directive at ${:04X}: {}", current_address, e))?;
                        // Little-endian: low byte first, then high byte
                        machine.push((val & 0xFF) as u8);
                        machine.push((val >> 8) as u8);
                        current_address = current_address.wrapping_add(2);
                    }
                }
                Item::String(s) => {
                    for byte in s.bytes() {
                        machine.push(byte);
                        current_address = current_address.wrapping_add(1);
                    }
                }
                Item::IncBin(filename) => {
                    let bytes = fs::read(filename)
                        .map_err(|e| format!(".incbin \"{}\" at ${:04X}: {}", filename, current_address, e))?;
                    for byte in bytes {
                        machine.push(byte);
                        current_address = current_address.wrapping_add(1);
                    }
                }
                Item::Instruction { mnemonic, operand } => {
                    let bytes = self.assemble_instruction(mnemonic, operand.as_deref(), current_address)
                        .map_err(|e| {
                            let op_str = operand.as_ref().map(|s| format!(" {}", s)).unwrap_or_default();
                            format!("${:04X}: {}{} - {}", current_address, mnemonic, op_str, e)
                        })?;
                    current_address = current_address.wrapping_add(bytes.len() as u16);
                    machine.extend_from_slice(&bytes);
                }
                Item::Pad(n) => {
                    for _ in 0..*n {
                        machine.push(0);
                        current_address = current_address.wrapping_add(1);
                    }
                }
            }
        }

        Ok((machine, instructions))
    }

    // ===== Instruction assembly =====

    pub fn assemble_instruction(
        &self,
        mnemonic: &str,
        operand: Option<&str>,
        current_address: u16,
    ) -> Result<Vec<u8>, String> {
        // Implied/accumulator form
        if operand.is_none() {
            if let Some(&op) = self.opcodes.opcodes.get(mnemonic) {
                return Ok(vec![op]);
            }
            return Err(format!("Unknown mnemonic: {}", mnemonic));
        }

        let operand_raw = operand.unwrap();
        let (operand, mode_override) = parse_addr_override(operand_raw);

        // Special handlers
        if mnemonic == "JMP" {
            return self.handle_jump(operand, current_address);
        }
        if mnemonic == "JSR" {
            return self.handle_subroutine(operand, current_address);
        }
        if is_branch(mnemonic) {
            return self.handle_branch(mnemonic, operand, current_address);
        }

        // Immediate mode: #value (can have expressions like #$02+1)
        if let Some(rest) = operand.strip_prefix('#') {
            let expr = ExpressionParser::parse(rest)?;
            let eval = ExpressionEvaluator::new(&self.symbols, current_address);
            let value = eval.evaluate_u16(&expr)?;
            if value > 0xFF {
                return Err(format!("Immediate value too large: ${:04X}", value));
            }
            return Ok(vec![
                *self
                    .opcodes
                    .opcodes
                    .get(mnemonic)
                    .ok_or_else(|| format!("Unknown mnemonic: {}", mnemonic))?,
                (value & 0xFF) as u8,
            ]);
        }

        // Indirect modes
        if operand.starts_with('(') {
            return self.handle_indirect(mnemonic, operand, current_address);
        }

        // Indexed addressing: addr,X or addr,Y
        if let Some((addr_part, idx)) = operand.split_once(',') {
            return self.handle_indexed(mnemonic, addr_part.trim(), idx.trim(), mode_override, current_address);
        }

        // Plain absolute/zeropage
        self.handle_absolute_or_zp(mnemonic, operand, mode_override, current_address)
    }

    fn handle_jump(&self, operand: &str, current_address: u16) -> Result<Vec<u8>, String> {
        if operand.starts_with('(') && operand.ends_with(')') {
            let inner = &operand[1..operand.len() - 1];
            let expr = ExpressionParser::parse(inner)?;
            let eval = ExpressionEvaluator::new(&self.symbols, current_address);
            let value = eval.evaluate_u16(&expr)?;
            return Ok(vec![0x6C, (value & 0xFF) as u8, (value >> 8) as u8]);
        }
        let expr = ExpressionParser::parse(operand)?;
        let eval = ExpressionEvaluator::new(&self.symbols, current_address);
        let value = eval.evaluate_u16(&expr)?;
        Ok(vec![0x4C, (value & 0xFF) as u8, (value >> 8) as u8])
    }

    fn handle_subroutine(&self, operand: &str, current_address: u16) -> Result<Vec<u8>, String> {
        let expr = ExpressionParser::parse(operand)?;
        let eval = ExpressionEvaluator::new(&self.symbols, current_address);
        let value = eval.evaluate_u16(&expr)?;
        Ok(vec![0x20, (value & 0xFF) as u8, (value >> 8) as u8])
    }

    fn handle_branch(
        &self,
        mnemonic: &str,
        operand: &str,
        current_address: u16,
    ) -> Result<Vec<u8>, String> {
        let target = self
            .symbols
            .get(operand)
            .ok_or_else(|| format!("Undefined label: {}", operand))?;
        let offset = target as i32 - (current_address as i32 + 2);
        if offset < -128 || offset > 127 {
            return Err(format!(
                "Branch offset out of range: {}. Target: ${:04X}, Current: ${:04X}",
                offset, target, current_address
            ));
        }
        let opcode = *self.opcodes.opcodes.get(mnemonic).unwrap();
        Ok(vec![opcode, (offset as i8) as u8])
    }

    fn handle_indirect(&self, mnemonic: &str, operand: &str, current_address: u16) -> Result<Vec<u8>, String> {
        // (addr),Y
        if operand.contains("),Y") {
            let inner = operand
                .strip_prefix('(')
                .and_then(|s| s.split("),Y").next())
                .unwrap_or("")
                .trim();
            let expr = ExpressionParser::parse(inner)?;
            let eval = ExpressionEvaluator::new(&self.symbols, current_address);
            let val = eval.evaluate_u16(&expr)?;
            let code = self
                .opcodes
                .extended_opcodes
                .get(mnemonic)
                .and_then(|m| m.get("indirect,Y"))
                .ok_or_else(|| format!("Unsupported mode for {}", mnemonic))?;
            return Ok(vec![*code, (val & 0xFF) as u8]);
        }
        // (addr,X)
        if operand.ends_with(')') {
            let inside = &operand[1..operand.len() - 1];
            let mut parts = inside.split(',').map(|s| s.trim());
            let a = parts.next().unwrap_or("");
            let idx = parts.next().unwrap_or("");
            if idx.eq_ignore_ascii_case("X") {
                let expr = ExpressionParser::parse(a)?;
                let eval = ExpressionEvaluator::new(&self.symbols, current_address);
                let val = eval.evaluate_u16(&expr)?;
                let code = self
                    .opcodes
                    .extended_opcodes
                    .get(mnemonic)
                    .and_then(|m| m.get("indirect,X"))
                    .ok_or_else(|| format!("Unsupported mode for {}", mnemonic))?;
                return Ok(vec![*code, (val & 0xFF) as u8]);
            }
        }
        Err("Invalid indirect addressing mode".to_string())
    }

    fn handle_indexed(
        &self,
        mnemonic: &str,
        addr_part: &str,
        idx: &str,
        mode_override: AddrOverride,
        current_address: u16,
    ) -> Result<Vec<u8>, String> {
        let expr = ExpressionParser::parse(addr_part)?;
        let eval = ExpressionEvaluator::new(&self.symbols, current_address);
        let val = eval.evaluate_u16(&expr)?;
        let force_zp = mode_override == AddrOverride::ForceZp;
        let force_abs = mode_override == AddrOverride::ForceAbs;
        let is_zp = val < 0x100;
        let mode_zp = format!("zeropage,{}", idx);
        let mode_abs = format!("absolute,{}", idx);

        if (is_zp && !force_abs) || force_zp {
            if let Some(code) = self
                .opcodes
                .extended_opcodes
                .get(mnemonic)
                .and_then(|m| m.get(mode_zp.as_str()))
            {
                return Ok(vec![*code, (val & 0xFF) as u8]);
            }
        }

        let code = self
            .opcodes
            .extended_opcodes
            .get(mnemonic)
            .and_then(|m| m.get(mode_abs.as_str()))
            .ok_or_else(|| format!("Unsupported mode for {}", mnemonic))?;
        Ok(vec![*code, (val & 0xFF) as u8, (val >> 8) as u8])
    }

    fn handle_absolute_or_zp(
        &self,
        mnemonic: &str,
        operand: &str,
        mode_override: AddrOverride,
        current_address: u16,
    ) -> Result<Vec<u8>, String> {
        let expr = ExpressionParser::parse(operand)?;
        let eval = ExpressionEvaluator::new(&self.symbols, current_address);
        let val = eval.evaluate_u16(&expr)?;
        let force_zp = mode_override == AddrOverride::ForceZp;
        let force_abs = mode_override == AddrOverride::ForceAbs;

        if (val < 0x100 && !force_abs) || force_zp {
            if let Some(code) = self
                .opcodes
                .extended_opcodes
                .get(mnemonic)
                .and_then(|m| m.get("zeropage"))
            {
                return Ok(vec![*code, (val & 0xFF) as u8]);
            }
        }

        let code = self
            .opcodes
            .extended_opcodes
            .get(mnemonic)
            .and_then(|m| m.get("absolute"))
            .ok_or_else(|| format!("Unsupported mode for {}", mnemonic))?;
        Ok(vec![*code, (val & 0xFF) as u8, (val >> 8) as u8])
    }

    // ===== Helpers =====

    fn instruction_size(&self, inst: &Item, current_address: u16) -> Result<usize, String> {
        match inst {
            Item::Instruction { mnemonic, operand } => {
                if let Ok(bytes) = self.assemble_instruction(mnemonic, operand.as_deref(), current_address) {
                    return Ok(bytes.len());
                }
                let m = mnemonic.as_str();
                if self.opcodes.opcodes.contains_key(m) && operand.is_none() {
                    return Ok(1);
                }
                if self.opcodes.opcodes.contains_key(m) && operand.is_some() {
                    if let Some(op) = operand {
                        if op.starts_with('#') {
                            return Ok(2);
                        }
                    }
                }
                if is_branch(m) {
                    return Ok(2);
                }
                Ok(3)
            }
            Item::Data(exprs) => Ok(exprs.len()),
            Item::Words(exprs) => Ok(exprs.len() * 2),  // 2 bytes per word
            Item::String(s) => Ok(s.len()),
            Item::IncBin(filename) => {
                // Try to get file size, or return error
                match fs::metadata(filename) {
                    Ok(metadata) => Ok(metadata.len() as usize),
                    Err(_) => Err(format!("Cannot read file: {}", filename)),
                }
            }
            Item::Pad(n) => Ok(*n),
            Item::Org(_) | Item::Label(_) | Item::Constant(_, _) => Ok(0),
        }
    }

    fn count_branches(&self, items: &[Item]) -> usize {
        items
            .iter()
            .filter(|it| match it {
                Item::Instruction { mnemonic, .. } => is_branch(mnemonic.as_str()),
                _ => false,
            })
            .count()
    }

    /// Sizing pre-pass: walk items to assign label and constant addresses.
    fn rebuild_symbols(&mut self, instructions: &[Item]) {
        self.symbols.clear();
        let mut current_address = self.start_address;
        for inst in instructions.iter() {
            match inst {
                Item::Label(name) => {
                    self.symbols.insert(name.clone(), current_address);
                }
                Item::Constant(name, expr) => {
                    let eval = ExpressionEvaluator::new(&self.symbols, current_address);
                    if let Ok(value) = eval.evaluate_u16(expr) {
                        self.symbols.insert(name.clone(), value);
                    }
                }
                Item::Org(expr) => {
                    let eval = ExpressionEvaluator::new(&self.symbols, current_address);
                    if let Ok(addr) = eval.evaluate_u16(expr) {
                        current_address = addr;
                    }
                }
                _ => {
                    if let Ok(size) = self.instruction_size(inst, current_address) {
                        current_address = current_address.wrapping_add(size as u16);
                    }
                }
            }
        }
    }

    /// Insert `JMP <end+1>` plus `$00`-fill before any reserved range the
    /// code would otherwise enter. Idempotent: existing skip-blocks in the
    /// input are passed through unchanged.
    pub fn apply_reserved_ranges(&mut self, instructions: &[Item]) -> Result<(Vec<Item>, bool), String> {
        if self.reserved_ranges.is_empty() {
            return Ok((instructions.to_vec(), false));
        }

        self.rebuild_symbols(instructions);

        const JMP_SIZE: u32 = 3;
        let mut output: Vec<Item> = Vec::with_capacity(instructions.len());
        let mut pc: u32 = self.start_address as u32;
        let mut modified = false;

        let mut i = 0usize;
        while i < instructions.len() {
            let inst = &instructions[i];

            match inst {
                Item::Label(_) | Item::Constant(_, _) => {
                    output.push(inst.clone());
                    i += 1;
                    continue;
                }
                Item::Org(expr) => {
                    let eval = ExpressionEvaluator::new(&self.symbols, pc as u16);
                    if let Ok(addr) = eval.evaluate_u16(expr) {
                        pc = addr as u32;
                    }
                    output.push(inst.clone());
                    i += 1;
                    continue;
                }
                _ => {}
            }

            // Pad and JMP-followed-by-Pad come from a prior run of this pass.
            // The parser never emits Pad, so this is a safe marker.
            let is_inserted = match inst {
                Item::Pad(_) => true,
                Item::Instruction { mnemonic, .. } if mnemonic == "JMP" => {
                    matches!(instructions.get(i + 1), Some(Item::Pad(_)))
                }
                _ => false,
            };

            if is_inserted {
                let size = self.instruction_size(inst, pc as u16).unwrap_or(0) as u32;
                output.push(inst.clone());
                pc = pc.wrapping_add(size);
                i += 1;
                continue;
            }

            for r in &self.reserved_ranges {
                if pc >= r.start as u32 && pc <= r.end as u32 {
                    return Err(format!(
                        "PC ${:04X} lands inside reserved range ${:04X}-${:04X}",
                        pc, r.start, r.end
                    ));
                }
            }

            let mut size = self.instruction_size(inst, pc as u16).unwrap_or(0) as u32;

            // Insert a skip-block before any range this item would either
            // cross or leave too little room (< JMP_SIZE bytes) before.
            loop {
                let conflict = self.reserved_ranges.iter().find(|r| {
                    let r_start = r.start as u32;
                    pc < r_start && pc + size > r_start.saturating_sub(JMP_SIZE)
                }).copied();

                let r = match conflict {
                    Some(r) => r,
                    None => break,
                };
                let r_start = r.start as u32;
                let r_end = r.end as u32;

                if pc + JMP_SIZE > r_start {
                    return Err(format!(
                        "Cannot fit 3-byte JMP before reserved ${:04X}-${:04X}: PC=${:04X}, only {} bytes available",
                        r.start, r.end, pc, r_start.saturating_sub(pc)
                    ));
                }

                let pre_pad = (r_start - JMP_SIZE) - pc;
                if pre_pad > 0 {
                    output.push(Item::Pad(pre_pad as usize));
                }
                output.push(Item::Instruction {
                    mnemonic: "JMP".to_string(),
                    operand: Some(format!("${:04X}", r_end + 1)),
                });
                output.push(Item::Pad((r_end - r_start + 1) as usize));

                pc = r_end + 1;
                modified = true;
                size = self.instruction_size(inst, pc as u16).unwrap_or(0) as u32;
            }

            output.push(inst.clone());
            pc = pc.wrapping_add(size);
            i += 1;
        }

        Ok((output, modified))
    }

    pub fn fix_long_branches(&mut self, instructions: &[Item]) -> (Vec<Item>, bool) {
        // CRITICAL: Build symbol table FIRST so we know where all labels are
        self.symbols.clear();
        let mut current_address = self.start_address;

        for inst in instructions.iter() {
            match inst {
                Item::Label(name) => {
                    self.symbols.insert(name.clone(), current_address);
                }
                Item::Constant(name, expr) => {
                    let eval = ExpressionEvaluator::new(&self.symbols, current_address);
                    if let Ok(value) = eval.evaluate_u16(expr) {
                        self.symbols.insert(name.clone(), value);
                    }
                }
                Item::Org(expr) => {
                    let eval = ExpressionEvaluator::new(&self.symbols, current_address);
                    if let Ok(addr) = eval.evaluate_u16(expr) {
                        current_address = addr;
                    }
                }
                _ => {
                    if let Ok(size) = self.instruction_size(inst, current_address) {
                        current_address = current_address.wrapping_add(size as u16);
                    }
                }
            }
        }

        // Now expand branches using the computed symbol table
        let mut fixed: Vec<Item> = Vec::new();
        current_address = self.start_address;
        let mut modified = false;

        for inst in instructions.iter() {
            // Handle ORG first
            if let Item::Org(expr) = inst {
                fixed.push(inst.clone());
                let eval = ExpressionEvaluator::new(&self.symbols, current_address);
                if let Ok(addr) = eval.evaluate_u16(expr) {
                    current_address = addr;
                }
                continue;
            }

            // Handle labels - they don't advance address
            if let Item::Label(_) = inst {
                fixed.push(inst.clone());
                continue;
            }

            // Handle constants - they don't advance address
            if let Item::Constant(_, _) = inst {
                fixed.push(inst.clone());
                continue;
            }

            // Check for branch expansion
            if let Item::Instruction { mnemonic, operand } = inst {
                if is_branch(mnemonic.as_str()) {
                    if let Some(op) = operand {
                        if let Some(target_addr) = self.symbols.get(op) {
                            let (_, in_range) =
                                self.calculate_branch_distance(current_address, target_addr);
                            if !in_range {
                                // Expand `BXX far_label` to:
                                //   BYY skip      ; YY = inverted condition
                                //   JMP far_label
                                //   skip:
                                // The branch must be INVERTED so that
                                // when the original BXX would have been
                                // taken (jump to far_label), control
                                // falls through into the JMP.
                                let inverted = invert_branch(mnemonic.as_str())
                                    .expect("is_branch implies invertible");
                                let skip_label = format!("__skip_{}", self.skip_label_counter);
                                self.skip_label_counter += 1;

                                // BYY __skip (2 bytes at current_address)
                                fixed.push(Item::Instruction {
                                    mnemonic: inverted.to_string(),
                                    operand: Some(skip_label.clone()),
                                });
                                current_address = current_address.wrapping_add(2);

                                // JMP label (3 bytes)
                                fixed.push(Item::Instruction {
                                    mnemonic: "JMP".to_string(),
                                    operand: Some(op.clone()),
                                });
                                current_address = current_address.wrapping_add(3);

                                // __skip: label (0 bytes - just marks position)
                                fixed.push(Item::Label(skip_label));

                                modified = true;
                                continue;
                            }
                        }
                    }
                }
            }

            // Add instruction as-is and advance address
            fixed.push(inst.clone());
            if let Ok(size) = self.instruction_size(inst, current_address) {
                current_address = current_address.wrapping_add(size as u16);
            }
        }

        (fixed, modified)
    }

    fn calculate_branch_distance(&self, from_addr: u16, to_addr: u16) -> (i16, bool) {
        let offset = to_addr as i32 - (from_addr as i32 + 2);
        (offset as i16, (-128..=127).contains(&(offset as i16)))
    }

    // ===== Listing (feature-gated) =====

    #[cfg(feature = "listing")]
    pub fn print_assembly_listing(&self, instructions: &[Item]) {
        let mut current_address = self.start_address;
        println!("\nAssembly Listing:");
        println!("Address:  Machine Code  Assembly");
        println!("{}", "-".repeat(50));
        for inst in instructions.iter() {
            match inst {
                Item::Label(name) => {
                    println!("${:04X}:          {}:", current_address, name);
                }
                Item::Constant(name, expr) => {
                    let eval = ExpressionEvaluator::new(&self.symbols, current_address);
                    if let Ok(value) = eval.evaluate_u16(expr) {
                        println!("              {} = ${:04X}", name, value);
                    }
                }
                Item::Instruction { mnemonic, operand } => {
                    if let Ok(size) = self.instruction_size(inst, current_address) {
                        let code_bytes = self
                            .assemble_instruction(mnemonic, operand.as_deref(), current_address)
                            .unwrap_or_else(|_| vec![]);
                        let hex_bytes = code_bytes
                            .iter()
                            .map(|b| format!("${:02X}", b))
                            .collect::<Vec<_>>()
                            .join(" ");
                        let hex_padded = format!("{:<12}", hex_bytes);
                        let op_str = operand.clone().unwrap_or_default();
                        println!(
                            "${:04X}: {} {} {}",
                            current_address, hex_padded, mnemonic, op_str
                        );
                        current_address = current_address.wrapping_add(size as u16);
                    }
                }
                Item::Org(expr) => {
                    let eval = ExpressionEvaluator::new(&self.symbols, current_address);
                    if let Ok(addr) = eval.evaluate_u16(expr) {
                        println!("${:04X}:          *=${:04X}", current_address, addr);
                        current_address = addr;
                    }
                }
                Item::Data(exprs) => {
                    let eval = ExpressionEvaluator::new(&self.symbols, current_address);
                    let bytes: Vec<u8> = exprs.iter()
                        .filter_map(|e| eval.evaluate_u16(e).ok())
                        .map(|v| (v & 0xFF) as u8)
                        .collect();
                    let hex_data = bytes
                        .iter()
                        .map(|b| format!("${:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let hex_padded = format!("{:<12}", hex_data.clone());
                    println!(
                        "${:04X}: {} .byte {}",
                        current_address, hex_padded, hex_data
                    );
                    current_address = current_address.wrapping_add(bytes.len() as u16);
                }
                Item::Words(exprs) => {
                    let eval = ExpressionEvaluator::new(&self.symbols, current_address);
                    let words: Vec<u16> = exprs.iter()
                        .filter_map(|e| eval.evaluate_u16(e).ok())
                        .collect();
                    let bytes: Vec<u8> = words.iter()
                        .flat_map(|&w| vec![(w & 0xFF) as u8, (w >> 8) as u8])
                        .collect();
                    let hex_data = bytes
                        .iter()
                        .map(|b| format!("${:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let hex_padded = format!("{:<12}", hex_data);
                    let word_data = words
                        .iter()
                        .map(|w| format!("${:04X}", w))
                        .collect::<Vec<_>>()
                        .join(",");
                    println!(
                        "${:04X}: {} .word {}",
                        current_address, hex_padded, word_data
                    );
                    current_address = current_address.wrapping_add(bytes.len() as u16);
                }
                Item::String(s) => {
                    let bytes: Vec<u8> = s.bytes().collect();
                    let hex_data = bytes
                        .iter()
                        .take(6)
                        .map(|b| format!("${:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let mut hex_padded = format!("{:<12}", hex_data);
                    if bytes.len() > 6 {
                        hex_padded = format!("{}...", hex_padded);
                    }
                    println!(
                        "${:04X}: {} .string \"{}\"",
                        current_address, hex_padded, s
                    );
                    current_address = current_address.wrapping_add(bytes.len() as u16);
                }
                Item::IncBin(filename) => {
                    if let Ok(bytes) = fs::read(filename) {
                        let hex_preview = bytes
                            .iter()
                            .take(6)
                            .map(|b| format!("${:02X}", b))
                            .collect::<Vec<_>>()
                            .join(" ");
                        let mut hex_padded = format!("{:<12}", hex_preview);
                        if bytes.len() > 6 {
                            hex_padded = format!("{}...", hex_padded);
                        }
                        println!(
                            "${:04X}: {} .incbin \"{}\" ({} bytes)",
                            current_address, hex_padded, filename, bytes.len()
                        );
                        current_address = current_address.wrapping_add(bytes.len() as u16);
                    }
                }
                Item::Pad(n) => {
                    println!(
                        "${:04X}: {:<12} <reserved fill, {} bytes of $00>",
                        current_address, "", n
                    );
                    current_address = current_address.wrapping_add(*n as u16);
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
                Item::Label(name) => {
                    writeln!(f, "${:04X}:          {}:", current_address, name)?;
                }
                Item::Constant(name, expr) => {
                    let eval = ExpressionEvaluator::new(&self.symbols, current_address);
                    if let Ok(value) = eval.evaluate_u16(expr) {
                        writeln!(f, "              {} = ${:04X}", name, value)?;
                    }
                }
                Item::Instruction { mnemonic, operand } => {
                    if let Ok(size) = self.instruction_size(inst, current_address) {
                        let code_bytes = self
                            .assemble_instruction(mnemonic, operand.as_deref(), current_address)
                            .unwrap_or_default();
                        let hex_bytes = code_bytes
                            .iter()
                            .map(|b| format!("${:02X}", b))
                            .collect::<Vec<_>>()
                            .join(" ");
                        let hex_padded = format!("{:<12}", hex_bytes);
                        let op_str = operand.clone().unwrap_or_default();
                        writeln!(
                            f,
                            "${:04X}: {} {} {}",
                            current_address, hex_padded, mnemonic, op_str
                        )?;
                        current_address = current_address.wrapping_add(size as u16);
                    }
                }
                Item::Org(expr) => {
                    let eval = ExpressionEvaluator::new(&self.symbols, current_address);
                    if let Ok(addr) = eval.evaluate_u16(expr) {
                        writeln!(f, "${:04X}:          *=${:04X}", current_address, addr)?;
                        current_address = addr;
                    }
                }
                Item::Data(exprs) => {
                    let eval = ExpressionEvaluator::new(&self.symbols, current_address);
                    let bytes: Vec<u8> = exprs.iter()
                        .filter_map(|e| eval.evaluate_u16(e).ok())
                        .map(|v| (v & 0xFF) as u8)
                        .collect();
                    let hex_data = bytes
                        .iter()
                        .map(|b| format!("${:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let hex_padded = format!("{:<12}", hex_data.clone());
                    writeln!(f, "${:04X}: {} .byte {}", current_address, hex_padded, hex_data)?;
                    current_address = current_address.wrapping_add(bytes.len() as u16);
                }
                Item::Words(exprs) => {
                    let eval = ExpressionEvaluator::new(&self.symbols, current_address);
                    let words: Vec<u16> = exprs.iter()
                        .filter_map(|e| eval.evaluate_u16(e).ok())
                        .collect();
                    let bytes: Vec<u8> = words.iter()
                        .flat_map(|&w| vec![(w & 0xFF) as u8, (w >> 8) as u8])
                        .collect();
                    let hex_data = bytes
                        .iter()
                        .map(|b| format!("${:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let hex_padded = format!("{:<12}", hex_data);
                    let word_data = words
                        .iter()
                        .map(|w| format!("${:04X}", w))
                        .collect::<Vec<_>>()
                        .join(",");
                    writeln!(f, "${:04X}: {} .word {}", current_address, hex_padded, word_data)?;
                    current_address = current_address.wrapping_add(bytes.len() as u16);
                }
                Item::String(s) => {
                    let bytes: Vec<u8> = s.bytes().collect();
                    let hex_data = bytes
                        .iter()
                        .take(6)
                        .map(|b| format!("${:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let mut hex_padded = format!("{:<12}", hex_data);
                    if bytes.len() > 6 {
                        hex_padded = format!("{}...", hex_padded);
                    }
                    writeln!(f, "${:04X}: {} .string \"{}\"", current_address, hex_padded, s)?;
                    current_address = current_address.wrapping_add(bytes.len() as u16);
                }
                Item::IncBin(filename) => {
                    if let Ok(bytes) = fs::read(filename) {
                        let hex_preview = bytes
                            .iter()
                            .take(6)
                            .map(|b| format!("${:02X}", b))
                            .collect::<Vec<_>>()
                            .join(" ");
                        let mut hex_padded = format!("{:<12}", hex_preview);
                        if bytes.len() > 6 {
                            hex_padded = format!("{}...", hex_padded);
                        }
                        writeln!(
                            f,
                            "${:04X}: {} .incbin \"{}\" ({} bytes)",
                            current_address, hex_padded, filename, bytes.len()
                        )?;
                        current_address = current_address.wrapping_add(bytes.len() as u16);
                    }
                }
                Item::Pad(n) => {
                    writeln!(
                        f,
                        "${:04X}: {:<12} <reserved fill, {} bytes of $00>",
                        current_address, "", n
                    )?;
                    current_address = current_address.wrapping_add(*n as u16);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod reserved_tests {
    use super::*;

    fn lda_program(count: usize) -> String {
        // Each "LDA #$00" is 2 bytes.
        let mut s = String::from("*=$0800\n");
        for _ in 0..count {
            s.push_str("LDA #$00\n");
        }
        s
    }

    #[test]
    fn no_reserved_ranges_is_unchanged() {
        let mut a = Assembler6502::new();
        let bytes = a.assemble_bytes("*=$0800\nLDA #$42\nSTA $0200\n").unwrap();
        assert_eq!(bytes, vec![0xA9, 0x42, 0x8D, 0x00, 0x02]);
    }

    #[test]
    fn add_reserved_validates_bounds() {
        let mut a = Assembler6502::new();
        assert!(a.add_reserved_range(0x0900, 0x08FF).is_err()); // start > end
        assert!(a.add_reserved_range(0x0100, 0xFFFF).is_err()); // end at top
        a.add_reserved_range(0x0900, 0x09FF).unwrap();
        // overlap
        assert!(a.add_reserved_range(0x09F0, 0x0A00).is_err());
        // gap < 3 (0x0A00 - 0x09FF - 1 = 0)
        assert!(a.add_reserved_range(0x0A00, 0x0A2F).is_err());
        // gap == 2
        assert!(a.add_reserved_range(0x0A02, 0x0A2F).is_err());
        // gap == 3 OK
        a.add_reserved_range(0x0A03, 0x0A2F).unwrap();
    }

    #[test]
    fn ranges_kept_sorted() {
        let mut a = Assembler6502::new();
        a.add_reserved_range(0x0B00, 0x0B4F).unwrap();
        a.add_reserved_range(0x0900, 0x09FF).unwrap();
        let r = a.reserved_ranges();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].start, 0x0900);
        assert_eq!(r[1].start, 0x0B00);
    }

    #[test]
    fn small_program_below_reserved_unchanged() {
        let mut a = Assembler6502::new();
        a.add_reserved_range(0x0900, 0x09FF).unwrap();
        let bytes = a.assemble_bytes(&lda_program(4)).unwrap();
        // 4 LDA #$00 = 8 bytes, doesn't reach $0900
        assert_eq!(bytes.len(), 8);
        assert!(bytes.iter().all(|&b| b == 0xA9 || b == 0x00));
    }

    #[test]
    fn skips_single_range_with_jmp_and_zero_fill() {
        let mut a = Assembler6502::new();
        a.add_reserved_range(0x0900, 0x09FF).unwrap();
        // 200 LDAs = 400 bytes, fills well past $0900
        let bytes = a.assemble_bytes(&lda_program(200)).unwrap();

        // Find the JMP $0A00 (4C 00 0A)
        let jmp_idx = bytes.windows(3).position(|w| w == [0x4C, 0x00, 0x0A])
            .expect("JMP $0A00 should be inserted");

        // The JMP must end at offset (0x0900 - 0x0800 - 1) = 0xFF.
        // So JMP starts at offset 0xFD, ends at 0xFF.
        assert_eq!(jmp_idx, 0xFD);

        // Bytes from $0900 to $09FF (offsets 0x100..=0x1FF) must be zero
        for off in 0x100..=0x1FF {
            assert_eq!(bytes[off], 0x00, "offset {:04X} should be $00", off);
        }
        // Code resumes at offset 0x200 (== $0A00)
        assert_eq!(bytes[0x200], 0xA9); // LDA opcode
    }

    #[test]
    fn nop_filled_program_skips_cleanly() {
        // 1-byte instructions: 253 NOPs end at $08FC, JMP at $08FD-$08FF, fill,
        // then the trailing LDA lands at $0A00.
        let mut a = Assembler6502::new();
        a.add_reserved_range(0x0900, 0x09FF).unwrap();
        let mut src = String::from("*=$0800\n");
        for _ in 0..253 { src.push_str("NOP\n"); }
        src.push_str("LDA #$11\n");
        let bytes = a.assemble_bytes(&src).unwrap();
        for off in 0..253 { assert_eq!(bytes[off], 0xEA); }
        assert_eq!(&bytes[253..256], &[0x4C, 0x00, 0x0A]);
        for off in 256..512 { assert_eq!(bytes[off], 0x00); }
        assert_eq!(&bytes[512..514], &[0xA9, 0x11]);
    }

    #[test]
    fn large_string_pushed_entirely_past_reserved() {
        let mut a = Assembler6502::new();
        a.add_reserved_range(0x0900, 0x09FF).unwrap();
        let mut src = String::from("*=$0800\n");
        for _ in 0..240 { src.push_str("NOP\n"); }
        src.push_str(".string \"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123\"\n");
        let bytes = a.assemble_bytes(&src).unwrap();

        let pos = bytes.windows(10).position(|w| w == b"ABCDEFGHIJ").unwrap();
        assert!(pos >= 0x200, "string at offset {:04X}, must be >= $0200", pos);
        for off in 0x100..=0x1FF { assert_eq!(bytes[off], 0x00); }
    }

    #[test]
    fn multiple_ranges_both_skipped() {
        let mut a = Assembler6502::new();
        a.add_reserved_range(0x0900, 0x09FF).unwrap();
        a.add_reserved_range(0x0B00, 0x0B4F).unwrap();
        let bytes = a.assemble_bytes(&lda_program(400)).unwrap();

        let jmp1 = bytes.windows(3).position(|w| w == [0x4C, 0x00, 0x0A]).unwrap();
        assert_eq!(jmp1, 0xFD);
        for off in 0x100..=0x1FF { assert_eq!(bytes[off], 0x00); }

        let jmp2 = bytes.windows(3).position(|w| w == [0x4C, 0x50, 0x0B]).unwrap();
        assert_eq!(jmp2, 0x2FD);
        for off in 0x300..=0x34F { assert_eq!(bytes[off], 0x00); }
    }

    #[test]
    fn assembling_twice_is_idempotent() {
        let mut a = Assembler6502::new();
        a.add_reserved_range(0x0900, 0x09FF).unwrap();
        let bytes1 = a.assemble_bytes(&lda_program(200)).unwrap();
        let bytes2 = a.assemble_bytes(&lda_program(200)).unwrap();
        assert_eq!(bytes1, bytes2);
    }

    #[test]
    fn long_branch_across_reserved_range() {
        let mut a = Assembler6502::new();
        a.add_reserved_range(0x0900, 0x09FF).unwrap();
        let mut src = String::from("*=$0800\nBEQ FAR\n");
        for _ in 0..200 { src.push_str("LDA #$00\n"); }
        src.push_str("FAR:\nNOP\n");
        let bytes = a.assemble_bytes(&src).unwrap();

        assert!(bytes.windows(3).any(|w| w == [0x4C, 0x00, 0x0A]));
        for off in 0x100..=0x1FF { assert_eq!(bytes[off], 0x00); }
    }

    #[test]
    fn no_jmp_when_code_ends_before_reserved() {
        let mut a = Assembler6502::new();
        a.add_reserved_range(0x0900, 0x09FF).unwrap();
        // Tiny program. No JMP should be inserted.
        let bytes = a.assemble_bytes("*=$0800\nLDA #$42\nRTS\n").unwrap();
        assert_eq!(bytes, vec![0xA9, 0x42, 0x60]);
    }

    #[test]
    fn org_into_reserved_range_errors() {
        let mut a = Assembler6502::new();
        a.add_reserved_range(0x0900, 0x09FF).unwrap();
        let res = a.assemble_bytes("*=$0950\nLDA #$00\n");
        assert!(res.is_err(), "ORG into reserved should fail");
    }

    #[test]
    fn clear_reserved_ranges_resets() {
        let mut a = Assembler6502::new();
        a.add_reserved_range(0x0900, 0x09FF).unwrap();
        assert_eq!(a.reserved_ranges().len(), 1);
        a.clear_reserved_ranges();
        assert_eq!(a.reserved_ranges().len(), 0);
        // After clearing, code below $0900 produces normal output.
        let bytes = a.assemble_bytes("*=$0800\nLDA #$42\n").unwrap();
        assert_eq!(bytes, vec![0xA9, 0x42]);
    }
}
