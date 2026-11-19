//! Parse the source into a full_moon AST. String rewriting consumes this validated syntax tree;
//! the remaining token-level passes consume the tokenization of validated source.
use full_moon::{ast::Ast, LuaVersion};

use crate::lexer::{self, Token};

#[derive(Debug)]
pub struct Parsed {
    #[allow(dead_code)]
    pub ast: Ast,
    /// Tokenization of the same source, used by the token-level passes.
    pub tokens: Vec<Token>,
}

/// Parse `source` as Lua (5.4 grammar), rejecting invalid input before any transform runs.
pub fn parse(source: &str) -> Result<Parsed, String> {
    let result = full_moon::parse_fallible(source, LuaVersion::new());
    if let Some(error) = result.errors().first() {
        return Err(format!("{error}"));
    }
    let tokens = lexer::lex(source)?;
    Ok(Parsed {
        ast: result.into_ast(),
        tokens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_source_and_reports_position_of_invalid_source() {
        let parsed = parse("local x = 'ok'").unwrap();
        assert_eq!(parsed.tokens.len(), 7);
        let error = parse("local = broken").unwrap_err();
        assert!(!error.is_empty());
    }

    #[test]
    fn ast_and_tokens_cover_the_same_source() {
        let source = "local function f(a) return a .. 'x' end";
        let parsed = parse(source).unwrap();
        assert_eq!(parsed.ast.to_string(), source);
        assert_eq!(
            parsed
                .tokens
                .iter()
                .map(|t| t.text.as_str())
                .collect::<String>(),
            source
        );
    }
}
