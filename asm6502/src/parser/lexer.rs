//! Lexer and parser for assembly source lines

use super::expression::{Expr, ExpressionParser};

#[derive(Clone, Debug)]
pub enum Item {
    Instruction {
        mnemonic: String,
        operand: Option<String>
    },
    Label(String),
    Constant(String, Expr),
    Data(Vec<Expr>),           // DCB and .byte
    Words(Vec<Expr>),          // .word (16-bit little-endian)
    String(String),            // .string "text"
    IncBin(String),            // .incbin "filename"
    Org(Expr),
}

#[derive(Clone, Debug)]
pub enum Either<T> {
    One(T),
    Many(Vec<T>),
}

/// Parse entire source into a list of Items
pub fn parse_source(source: &str) -> Result<Vec<Item>, String> {
    let mut instructions = Vec::new();
    for (line_num, raw) in source.lines().enumerate() {
        let line = raw.split(';').next().unwrap_or("").trim().to_string();
        if line.is_empty() {
            continue;
        }
        match parse_line(&line) {
            Ok(Some(parsed)) => {
                match parsed {
                    Either::Many(list) => instructions.extend(list),
                    Either::One(item) => instructions.push(item),
                }
            }
            Ok(None) => {
                // Empty line or comment only - skip
            }
            Err(e) => {
                return Err(format!("Line {}: {} - {}", line_num + 1, line, e));
            }
        }
    }
    Ok(instructions)
}

/// Parse a single line into an Item
pub fn parse_line(line: &str) -> Result<Option<Either<Item>>, String> {
    let l = line.split(';').next().unwrap_or("").trim();
    if l.is_empty() {
        return Ok(None);
    }

    // Label with DCB on same line: "label: DCB $01 $02"
    if l.contains(':') && l.contains("DCB") {
        let mut parts = l.split(':');
        let label = parts.next().unwrap().trim().to_string();
        let rest = parts.next().unwrap_or("").trim();
        if rest.starts_with("DCB") {
            let data_exprs: Vec<Expr> = rest[3..]
                .split_whitespace()
                .map(|s| ExpressionParser::parse(s))
                .collect::<Result<_, _>>()?;
            return Ok(Some(Either::Many(vec![
                Item::Label(label),
                Item::Data(data_exprs),
            ])));
        }
    }

    // Simple label: "label:"
    if l.ends_with(':') {
        return Ok(Some(Either::One(Item::Label(
            l[..l.len() - 1].to_string(),
        ))));
    }

    // Constant assignment: "LABEL = value" or "LABEL = *+1"
    if l.contains('=') && !l.starts_with('*') {
        let parts: Vec<&str> = l.splitn(2, '=').collect();
        if parts.len() == 2 {
            let name = parts[0].trim();
            let value_str = parts[1].trim();

            // Validate label name
            if !name.is_empty() && name.chars().next().unwrap().is_ascii_alphabetic() {
                let expr = ExpressionParser::parse(value_str)?;
                return Ok(Some(Either::One(Item::Constant(name.to_string(), expr))));
            }
        }
    }

    // Origin directive: "*=$0800"
    if let Some(rest) = l.strip_prefix("*=") {
        let expr = ExpressionParser::parse(rest.trim())?;
        return Ok(Some(Either::One(Item::Org(expr))));
    }

    // .byte directive: ".byte $01,$02,$03"
    if let Some(rest) = l.strip_prefix(".byte") {
        let data: Vec<Expr> = rest
            .split(',')
            .map(|s| ExpressionParser::parse(s.trim()))
            .collect::<Result<_, _>>()?;
        return Ok(Some(Either::One(Item::Data(data))));
    }

    // .word directive: ".word $1000,$2000"
    if let Some(rest) = l.strip_prefix(".word") {
        let words: Vec<Expr> = rest
            .split(',')
            .map(|s| ExpressionParser::parse(s.trim()))
            .collect::<Result<_, _>>()?;
        return Ok(Some(Either::One(Item::Words(words))));
    }

    // .string directive: ".string "hello""
    if let Some(rest) = l.strip_prefix(".string") {
        let rest = rest.trim();
        if rest.starts_with('"') && rest.ends_with('"') {
            let string_content = &rest[1..rest.len() - 1];
            return Ok(Some(Either::One(Item::String(string_content.to_string()))));
        }
        return Err("Invalid .string format, expected quotes".to_string());
    }

    // .incbin directive: ".incbin "filename.bin""
    if let Some(rest) = l.strip_prefix(".incbin") {
        let rest = rest.trim();
        if rest.starts_with('"') && rest.ends_with('"') {
            let filename = &rest[1..rest.len() - 1];
            return Ok(Some(Either::One(Item::IncBin(filename.to_string()))));
        }
        return Err("Invalid .incbin format, expected quotes".to_string());
    }

    // Data directive: "DCB $01 $02 $03"
    if l.starts_with("DCB") {
        let data: Vec<Expr> = l[3..]
            .split_whitespace()
            .map(|s| ExpressionParser::parse(s))
            .collect::<Result<_, _>>()?;
        return Ok(Some(Either::One(Item::Data(data))));
    }

    // Instruction: "LDA #$42" or "NOP"
    // IMPORTANT: Split on FIRST whitespace only, to allow spaces in operands
    let parts: Vec<&str> = l.splitn(2, char::is_whitespace).collect();
    match parts.len() {
        1 => Ok(Some(Either::One(Item::Instruction {
            mnemonic: parts[0].to_string(),
            operand: None,
        }))),
        2 => {
            // Keep operand as string with all spaces intact
            Ok(Some(Either::One(Item::Instruction {
                mnemonic: parts[0].to_string(),
                operand: Some(parts[1].trim().to_string()),
            })))
        }
        _ => Err(format!("Invalid line: {}", l)),
    }
}
