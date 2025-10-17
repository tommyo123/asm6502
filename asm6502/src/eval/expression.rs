//! Expression evaluation with symbol resolution

use crate::parser::expression::Expr;
use crate::symbol::SymbolTable;

pub struct ExpressionEvaluator<'a> {
    symbols: &'a SymbolTable,
    current_address: u16,
}

impl<'a> ExpressionEvaluator<'a> {
    pub fn new(symbols: &'a SymbolTable, current_address: u16) -> Self {
        Self {
            symbols,
            current_address,
        }
    }

    /// Evaluate an expression to a u32 value
    /// Returns u32 to handle intermediate calculations like $10000 - offset
    pub fn evaluate(&self, expr: &Expr) -> Result<u32, String> {
        match expr {
            Expr::Number(n) => Ok(*n),

            Expr::Label(name) => {
                self.symbols
                    .get(name)
                    .map(|v| v as u32)
                    .ok_or_else(|| format!("Undefined label: {}", name))
            }

            Expr::CurrentAddress => Ok(self.current_address as u32),

            Expr::Immediate(inner) => {
                // Immediate mode - evaluate the inner expression
                self.evaluate(inner)
            }

            Expr::LowByte(inner) => {
                // Extract low byte (bits 0-7)
                let value = self.evaluate(inner)?;
                Ok(value & 0xFF)
            }

            Expr::HighByte(inner) => {
                // Extract high byte (bits 8-15)
                let value = self.evaluate(inner)?;
                Ok((value >> 8) & 0xFF)
            }

            Expr::Add(left, right) => {
                let l = self.evaluate(left)?;
                let r = self.evaluate(right)?;
                Ok(l.wrapping_add(r))
            }

            Expr::Sub(left, right) => {
                let l = self.evaluate(left)?;
                let r = self.evaluate(right)?;
                Ok(l.wrapping_sub(r))
            }

            Expr::Mul(left, right) => {
                let l = self.evaluate(left)?;
                let r = self.evaluate(right)?;
                Ok(l.wrapping_mul(r))
            }

            Expr::Div(left, right) => {
                let l = self.evaluate(left)?;
                let r = self.evaluate(right)?;
                if r == 0 {
                    return Err("Division by zero".to_string());
                }
                Ok(l / r)
            }
        }
    }

    /// Evaluate and convert to u16 (with wrapping for values > 0xFFFF)
    pub fn evaluate_u16(&self, expr: &Expr) -> Result<u16, String> {
        let value = self.evaluate(expr)?;
        Ok(value as u16)
    }

    /// Try to evaluate, returning None if labels are undefined (forward reference)
    #[allow(dead_code)]
    pub fn try_evaluate(&self, expr: &Expr) -> Option<u32> {
        self.evaluate(expr).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::SymbolTable;

    #[test]
    fn test_evaluate_number() {
        let symbols = SymbolTable::new();
        let evaluator = ExpressionEvaluator::new(&symbols, 0x1000);

        assert_eq!(evaluator.evaluate(&Expr::Number(42)).unwrap(), 42);
        assert_eq!(evaluator.evaluate(&Expr::Number(0x10000)).unwrap(), 0x10000);
    }

    #[test]
    fn test_evaluate_current_address() {
        let symbols = SymbolTable::new();
        let evaluator = ExpressionEvaluator::new(&symbols, 0x1000);

        assert_eq!(evaluator.evaluate(&Expr::CurrentAddress).unwrap(), 0x1000);
    }

    #[test]
    fn test_evaluate_label() {
        let mut symbols = SymbolTable::new();
        symbols.insert("LABEL".to_string(), 0x2000);
        let evaluator = ExpressionEvaluator::new(&symbols, 0x1000);

        assert_eq!(evaluator.evaluate(&Expr::Label("LABEL".to_string())).unwrap(), 0x2000);
    }

    #[test]
    fn test_evaluate_add() {
        let mut symbols = SymbolTable::new();
        symbols.insert("LABEL".to_string(), 0x2000);
        let evaluator = ExpressionEvaluator::new(&symbols, 0x1000);

        let expr = Expr::Add(
            Box::new(Expr::Label("LABEL".to_string())),
            Box::new(Expr::Number(1)),
        );

        assert_eq!(evaluator.evaluate(&expr).unwrap(), 0x2001);
    }

    #[test]
    fn test_evaluate_current_plus_offset() {
        let symbols = SymbolTable::new();
        let evaluator = ExpressionEvaluator::new(&symbols, 0x1000);

        let expr = Expr::Add(
            Box::new(Expr::CurrentAddress),
            Box::new(Expr::Number(2)),
        );

        assert_eq!(evaluator.evaluate(&expr).unwrap(), 0x1002);
    }

    #[test]
    fn test_undefined_label() {
        let symbols = SymbolTable::new();
        let evaluator = ExpressionEvaluator::new(&symbols, 0x1000);

        let expr = Expr::Label("UNDEFINED".to_string());
        assert!(evaluator.evaluate(&expr).is_err());
    }

    #[test]
    fn test_u32_overflow() {
        let symbols = SymbolTable::new();
        let evaluator = ExpressionEvaluator::new(&symbols, 0x1000);

        // $10000 - $100 = $FF00
        let expr = Expr::Sub(
            Box::new(Expr::Number(0x10000)),
            Box::new(Expr::Number(0x100)),
        );

        assert_eq!(evaluator.evaluate(&expr).unwrap(), 0xFF00);
        assert_eq!(evaluator.evaluate_u16(&expr).unwrap(), 0xFF00);
    }

    #[test]
    fn test_low_high_byte() {
        let symbols = SymbolTable::new();
        let evaluator = ExpressionEvaluator::new(&symbols, 0x1000);

        let expr_low = Expr::LowByte(Box::new(Expr::Number(0x1234)));
        assert_eq!(evaluator.evaluate(&expr_low).unwrap(), 0x34);

        let expr_high = Expr::HighByte(Box::new(Expr::Number(0x1234)));
        assert_eq!(evaluator.evaluate(&expr_high).unwrap(), 0x12);
    }
}
