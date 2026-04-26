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

/// The branch with the opposite condition. Used by the long-branch
/// expander: a `BXX far_label` becomes `BYY skip; JMP far_label; skip:`
/// where YY inverts XX so the jump fires under the original condition.
pub fn invert_branch(mnemonic: &str) -> Option<&'static str> {
    Some(match mnemonic {
        "BCC" => "BCS",
        "BCS" => "BCC",
        "BEQ" => "BNE",
        "BNE" => "BEQ",
        "BMI" => "BPL",
        "BPL" => "BMI",
        "BVC" => "BVS",
        "BVS" => "BVC",
        _ => return None,
    })
}