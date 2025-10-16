//! Error types for the assembler

use std::fmt;

#[derive(Debug)]
pub enum AsmError {
    Asm(String),
    Io(std::io::Error),
}

impl fmt::Display for AsmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AsmError::Asm(msg) => write!(f, "Assembly error: {}", msg),
            AsmError::Io(err) => write!(f, "IO error: {}", err),
        }
    }
}

impl std::error::Error for AsmError {}

impl From<std::io::Error> for AsmError {
    fn from(e: std::io::Error) -> Self {
        AsmError::Io(e)
    }
}
