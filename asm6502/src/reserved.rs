//! Reserved memory ranges - regions skipped during assembly.
//!
//! When the program counter would otherwise emit bytes into a reserved
//! range, the assembler inserts a `JMP <end+1>` and zero-fills the range.
//! Indivisible blocks (`.string`, `.byte`, `.word`, `.incbin`) that would
//! cross a range are pushed past it in their entirety.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReservedRange {
    pub start: u16,
    pub end: u16,
}

impl ReservedRange {
    pub fn new(start: u16, end: u16) -> Self {
        Self { start, end }
    }

    pub fn size(&self) -> u32 {
        self.end as u32 - self.start as u32 + 1
    }

    pub fn contains(&self, addr: u16) -> bool {
        addr >= self.start && addr <= self.end
    }
}
