//! Expression parsing for assembly operands

use super::number::NumberParser;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(u16),
    Label(String),
    CurrentAddress,  // * symbol
    Immediate(Box<Expr>),  // #value - immediate addressing mode
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
}

pub struct ExpressionParser;

impl ExpressionParser {
    /// Parse an expression string
    pub fn parse(s: &str) -> Result<Expr, String> {
        let s = s.trim();

        // Handle immediate mode prefix (#) - strip it, we'll handle it in assembler
        if let Some(rest) = s.strip_prefix('#') {
            let inner = Self::parse(rest.trim())?;
            return Ok(Expr::Immediate(Box::new(inner)));
        }

        // Check for current address symbol
        if s == "*" {
            return Ok(Expr::CurrentAddress);
        }

        // Check if it contains operators - if so, parse as expression
        if s.contains('+') || s.contains('-') || s.contains('*') || s.contains('/') || s.contains('(') || s.contains(')') {
            // Has operators - parse as expression
            return Self::parse_additive(s);
        }

        // Try to parse as simple number
        if let Ok(num) = NumberParser::parse(s) {
            return Ok(Expr::Number(num));
        }

        // Check if it's a simple label (no operators)
        if Self::is_valid_label(s) {
            return Ok(Expr::Label(s.to_string()));
        }

        Err(format!("Invalid expression: {}", s))
    }

    /// Parse addition and subtraction (lowest precedence)
    fn parse_additive(s: &str) -> Result<Expr, String> {
        // Find rightmost + or - that's not inside parentheses
        let mut depth = 0;
        let mut op_pos = None;
        let mut op_char = '\0';

        for (i, ch) in s.char_indices().rev() {
            match ch {
                ')' => depth += 1,
                '(' => depth -= 1,
                '+' | '-' if depth == 0 => {
                    op_pos = Some(i);
                    op_char = ch;
                    break;
                }
                _ => {}
            }
        }

        if let Some(pos) = op_pos {
            // Left side can be another additive expression for left-associativity
            let left = Self::parse_additive(&s[..pos])?;
            let right = Self::parse_multiplicative(&s[pos + 1..])?;
            return match op_char {
                '+' => Ok(Expr::Add(Box::new(left), Box::new(right))),
                '-' => Ok(Expr::Sub(Box::new(left), Box::new(right))),
                _ => unreachable!(),
            };
        }

        // No +/-, try multiplicative
        Self::parse_multiplicative(s)
    }

    /// Parse multiplication and division (higher precedence)
    fn parse_multiplicative(s: &str) -> Result<Expr, String> {
        // Find rightmost * or / that's not inside parentheses
        let mut depth = 0;
        let mut op_pos = None;
        let mut op_char = '\0';

        for (i, ch) in s.char_indices().rev() {
            match ch {
                ')' => depth += 1,
                '(' => depth -= 1,
                '*' | '/' if depth == 0 => {
                    // Check if * is current address (at start or after operator)
                    if ch == '*' && (i == 0 || matches!(s.chars().nth(i.saturating_sub(1)), Some('+' | '-' | '*' | '/' | '(' | ','))) {
                        continue; // This is current address, not multiply
                    }
                    op_pos = Some(i);
                    op_char = ch;
                    break;
                }
                _ => {}
            }
        }

        if let Some(pos) = op_pos {
            let left = Self::parse_primary(&s[..pos])?;
            let right = Self::parse_primary(&s[pos + 1..])?;
            return match op_char {
                '*' => Ok(Expr::Mul(Box::new(left), Box::new(right))),
                '/' => Ok(Expr::Div(Box::new(left), Box::new(right))),
                _ => unreachable!(),
            };
        }

        // No */, parse as primary
        Self::parse_primary(s)
    }

    /// Parse primary expression (number, label, *, or parenthesized expression)
    fn parse_primary(s: &str) -> Result<Expr, String> {
        let s = s.trim();

        // Parenthesized expression
        if s.starts_with('(') && s.ends_with(')') {
            return Self::parse(&s[1..s.len() - 1]);
        }

        // Current address
        if s == "*" {
            return Ok(Expr::CurrentAddress);
        }

        // Number
        if let Ok(num) = NumberParser::parse(s) {
            return Ok(Expr::Number(num));
        }

        // Label
        if Self::is_valid_label(s) {
            return Ok(Expr::Label(s.to_string()));
        }

        Err(format!("Invalid expression: {}", s))
    }

    /// Check if a string is a valid label name
    fn is_valid_label(s: &str) -> bool {
        if s.is_empty() {
            return false;
        }

        let mut chars = s.chars();
        let first = chars.next().unwrap();

        // First character must be letter or underscore
        if !first.is_ascii_alphabetic() && first != '_' {
            return false;
        }

        // Rest can be alphanumeric or underscore
        chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_number() {
        assert_eq!(ExpressionParser::parse("$FF").unwrap(), Expr::Number(255));
        assert_eq!(ExpressionParser::parse("255").unwrap(), Expr::Number(255));
        assert_eq!(ExpressionParser::parse("%11111111").unwrap(), Expr::Number(255));
    }

    #[test]
    fn test_simple_label() {
        assert_eq!(ExpressionParser::parse("LABEL").unwrap(), Expr::Label("LABEL".to_string()));
        assert_eq!(ExpressionParser::parse("my_label").unwrap(), Expr::Label("my_label".to_string()));
    }

    #[test]
    fn test_current_address() {
        assert_eq!(ExpressionParser::parse("*").unwrap(), Expr::CurrentAddress);
    }

    #[test]
    fn test_addition() {
        let expr = ExpressionParser::parse("LABEL+1").unwrap();
        match expr {
            Expr::Add(left, right) => {
                assert_eq!(*left, Expr::Label("LABEL".to_string()));
                assert_eq!(*right, Expr::Number(1));
            }
            _ => panic!("Expected Add expression"),
        }
    }

    #[test]
    fn test_subtraction() {
        let expr = ExpressionParser::parse("*-2").unwrap();
        match expr {
            Expr::Sub(left, right) => {
                assert_eq!(*left, Expr::CurrentAddress);
                assert_eq!(*right, Expr::Number(2));
            }
            _ => panic!("Expected Sub expression"),
        }
    }

    #[test]
    fn test_complex_expression() {
        let expr = ExpressionParser::parse("LABEL+$10-2").unwrap();
        match expr {
            Expr::Sub(left, right) => {
                match *left {
                    Expr::Add(l, r) => {
                        assert_eq!(*l, Expr::Label("LABEL".to_string()));
                        assert_eq!(*r, Expr::Number(0x10));
                    }
                    _ => panic!("Expected Add in left"),
                }
                assert_eq!(*right, Expr::Number(2));
            }
            _ => panic!("Expected Sub expression"),
        }
    }

    #[test]
    fn test_multiplication() {
        let expr = ExpressionParser::parse("10*2").unwrap();
        match expr {
            Expr::Mul(left, right) => {
                assert_eq!(*left, Expr::Number(10));
                assert_eq!(*right, Expr::Number(2));
            }
            _ => panic!("Expected Mul expression"),
        }
    }
}
