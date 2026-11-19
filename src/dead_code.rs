//! Inserts unreachable statement-level branches with literal-false guards.
use crate::lexer::{Kind, Token};
use crate::renamer::fresh_identifier;
use rand::RngCore;
use std::collections::HashSet;
pub fn transform(tokens: &mut Vec<Token>, rng: &mut impl RngCore) -> Result<usize, String> {
    if tokens.is_empty() {
        return Ok(0);
    }
    let mut used: HashSet<String> = tokens
        .iter()
        .filter(|t| t.kind == Kind::Ident)
        .map(|t| t.text.clone())
        .collect();
    let name = fresh_identifier(rng, &mut used);
    // At the start of the chunk, the false branch is never entered; its private local has no observable value.
    let src = format!("if false then local {name}=173*941+29 end;");
    let mut prefix = crate::lexer::lex(&src)?;
    prefix.extend(std::mem::take(tokens));
    *tokens = prefix;
    Ok(1)
}
