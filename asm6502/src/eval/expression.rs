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

    /// Evaluate an expression to a u16 value
    pub fn evaluate(&self, expr: &Expr) -> Result<u16, String> {
        match expr {
            Expr::Number(n) => Ok(*n),

            Expr::Label(name) => {
                self.symbols
                    .get(name)
                    .ok_or_else(|| format!("Undefined label: {}", name))
            }

            Expr::CurrentAddress => Ok(self.current_address),

            Expr::Immediate(inner) => {
                // Immediate mode - evaluate the inner expression
                self.evaluate(inner)
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

    /// Try to evaluate, returning None if labels are undefined (forward reference)
    #[allow(dead_code)]
    pub fn try_evaluate(&self, expr: &Expr) -> Option<u16> {
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
}
