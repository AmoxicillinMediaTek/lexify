//! Parser boundary: syntax comes from full_moon's Lua AST. Transform passes currently consume
//! its validated source tokenization; unsafe/incomplete CFG rewrites are intentionally not guessed.
use full_moon::ast::Ast;
#[derive(Debug)] pub struct Parsed { pub _ast: Ast }
