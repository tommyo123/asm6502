//! Addressing mode detection and handling

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum AddrOverride {
    Auto,
    ForceZp,
    ForceAbs,
}

/// Parse operand prefix for address mode override
pub fn parse_addr_override(operand: &str) -> (&str, AddrOverride) {
    if let Some(s) = operand.strip_prefix('<') {
        (s.trim(), AddrOverride::ForceZp)
    } else if let Some(s) = operand.strip_prefix('>') {
        (s.trim(), AddrOverride::ForceAbs)
    } else {
        (operand, AddrOverride::Auto)
    }
}

/// Check if a mnemonic is a branch instruction
pub fn is_branch(mnemonic: &str) -> bool {
    matches!(
        mnemonic,
        "BCC" | "BCS" | "BEQ" | "BMI" | "BNE" | "BPL" | "BVC" | "BVS"
    )
}