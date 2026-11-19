use crate::lexer::{Kind, Token};
/// Replace boolean literals in expression positions with equivalent truthiness expressions.
pub fn transform(tokens: &mut [Token]) {
    for t in tokens.iter_mut() {
        if t.kind == Kind::Ident && t.text == "true" {
            t.text = "(not not 1)".into();
        } else if t.kind == Kind::Ident && t.text == "false" {
            t.text = "(not 1)".into();
        }
    }
}
