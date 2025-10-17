//! Number parsing with multiple format support

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum NumberFormat {
    Hexadecimal,  // $FF, 0xFF
    Binary,       // %11111111, 0b11111111
    Decimal,      // 255
}

pub struct NumberParser;

impl NumberParser {
    /// Parse a number string in any supported format
    /// Returns u32 to support values > 65535 (like $10000)
    pub fn parse(s: &str) -> Result<u32, String> {
        let trimmed = s.trim();

        // Hexadecimal: $FF or 0xFF or 0xFFh
        if let Some(hex) = trimmed.strip_prefix('$') {
            return Self::parse_hex(hex);
        }
        if let Some(hex) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
            let hex = hex.strip_suffix('h').or(Some(hex)).unwrap();
            return Self::parse_hex(hex);
        }

        // Binary: %11111111 or 0b11111111
        if let Some(bin) = trimmed.strip_prefix('%') {
            return Self::parse_binary(bin);
        }
        if let Some(bin) = trimmed.strip_prefix("0b").or_else(|| trimmed.strip_prefix("0B")) {
            return Self::parse_binary(bin);
        }

        // Decimal: 255 (default if no prefix)
        Self::parse_decimal(trimmed)
    }

    /// Parse hexadecimal (without prefix)
    fn parse_hex(s: &str) -> Result<u32, String> {
        u32::from_str_radix(s, 16)
            .map_err(|_| format!("Invalid hexadecimal: {}", s))
    }

    /// Parse binary (without prefix)
    fn parse_binary(s: &str) -> Result<u32, String> {
        u32::from_str_radix(s, 2)
            .map_err(|_| format!("Invalid binary: {}", s))
    }

    /// Parse decimal
    fn parse_decimal(s: &str) -> Result<u32, String> {
        s.parse::<u32>()
            .map_err(|_| format!("Invalid decimal: {}", s))
    }

    /// Detect the format of a number string
    #[allow(dead_code)]
    pub fn detect_format(s: &str) -> NumberFormat {
        let trimmed = s.trim();
        if trimmed.starts_with('$') || trimmed.starts_with("0x") || trimmed.starts_with("0X") {
            NumberFormat::Hexadecimal
        } else if trimmed.starts_with('%') || trimmed.starts_with("0b") || trimmed.starts_with("0B") {
            NumberFormat::Binary
        } else {
            NumberFormat::Decimal
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_formats() {
        assert_eq!(NumberParser::parse("$FF").unwrap(), 255);
        assert_eq!(NumberParser::parse("$ff").unwrap(), 255);
        assert_eq!(NumberParser::parse("0xFF").unwrap(), 255);
        assert_eq!(NumberParser::parse("0xFFh").unwrap(), 255);
        assert_eq!(NumberParser::parse("$1234").unwrap(), 0x1234);
        assert_eq!(NumberParser::parse("$10000").unwrap(), 0x10000);
    }

    #[test]
    fn test_binary_formats() {
        assert_eq!(NumberParser::parse("%11111111").unwrap(), 255);
        assert_eq!(NumberParser::parse("0b11111111").unwrap(), 255);
        assert_eq!(NumberParser::parse("%10101010").unwrap(), 0xAA);
    }

    #[test]
    fn test_decimal() {
        assert_eq!(NumberParser::parse("255").unwrap(), 255);
        assert_eq!(NumberParser::parse("0").unwrap(), 0);
        assert_eq!(NumberParser::parse("65535").unwrap(), 65535);
        assert_eq!(NumberParser::parse("65536").unwrap(), 65536);
    }

    #[test]
    fn test_format_detection() {
        assert_eq!(NumberParser::detect_format("$FF"), NumberFormat::Hexadecimal);
        assert_eq!(NumberParser::detect_format("%11111111"), NumberFormat::Binary);
        assert_eq!(NumberParser::detect_format("255"), NumberFormat::Decimal);
    }
}
