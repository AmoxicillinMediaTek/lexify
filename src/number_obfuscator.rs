use crate::lexer::{Kind, Token};
/// Rewrites safe positive decimal integer tokens as parenthesized additive identities.
pub fn transform(tokens: &mut [Token]) {
    for t in tokens.iter_mut().filter(|t| t.kind == Kind::Number) {
        let raw = t.text.replace('_', "");
        if raw.bytes().all(|b| b.is_ascii_digit()) {
            if let Ok(n) = raw.parse::<u32>() {
                if (1..=1_000_000).contains(&n) {
                    t.text = format!("({}+1)", n - 1);
                } else if n == 0 {
                    t.text = "(1-1)".into();
                }
            }
        }
    }
}
